#!/bin/bash
# Shell entry point with a Python supervisor for argv-safe execution, timed
# notifications and signal forwarding to the whole build process group.
set -euo pipefail
exec python3 - "$0" "$@" <<'PY'
import argparse
import datetime
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

helper = str(Path(sys.argv.pop(1)).resolve())
sys.path.insert(0, str(Path(helper).parent))
sys.dont_write_bytecode = True
from screen_chat_notify import check_channel, default_socket, send_message


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def launch(arguments):
    parser = argparse.ArgumentParser(prog='screen-startscript.sh',
        description='Run a command in detached Screen and report to this Codex chat.',
        epilog='Example: ./screen-startscript.sh --build-path /path/to/repo -- cargo build\n'
        'Multiple commands: -- sh -c \'command1 && command2\'\n'
        'Requires bash, Python 3, GNU Screen and a running desktop app owning this chat.\n'
        'The starter prints launch JSON and exits; the helper runs inside Screen.\n'
        'Notifications use the versioned local Desktop IPC protocol without Node,\n'
        'a bridge or a queue. Active chats are steered; inactive chats get a new\n'
        'turn using their existing settings. No turn ID is needed.\n'
        'The protocol is internal and may change with desktop app updates.\n'
        'Launch exit status covers startup; inspect status.json for the build result\n'
        'and notification acceptance. Actual receipt is confirmed by the agent.\n'
        'Keep the chat turn open until all its Screen sessions have closed. If no\n'
        'message arrives for 300 seconds after launch/last alive, inspect Screen,\n'
        'the process and logs. This script does not enforce the chat turn policy.\n'
        'SIGKILL/host shutdown cannot send a final message. Native Cargo still\n'
        'requires the repository serial lock and configured canonical target.',
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--build-path', '--cwd', default=os.getcwd(), help='Command working directory')
    parser.add_argument('--thread', default=os.environ.get('CODEX_THREAD_ID'), help='Codex chat UUID (default: CODEX_THREAD_ID)')
    parser.add_argument('--name', help='Unique Screen name (default: chat prefix plus random ID)')
    parser.add_argument('--state-root', default='/private/tmp', help='Parent of private per-run logs/state directory')
    parser.add_argument('--heartbeat-seconds', type=float, default=180, help='alive interval; default 180, smaller values for tests')
    parser.add_argument('--notify-timeout', type=float, default=20, help='Maximum seconds per direct notification')
    parser.add_argument('--ipc-socket', default=default_socket(), help='Local Desktop IPC socket (default: CODEX_HOME/ipc/ipc.sock)')
    parser.add_argument('--env', action='append', default=[], metavar='NAME=VALUE', help='Additional build environment variable; repeatable')
    parser.add_argument('command', nargs=argparse.REMAINDER, help='Executable and arguments after --')
    args = parser.parse_args(arguments)
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not args.thread or not command:
        parser.error('--thread (or CODEX_THREAD_ID) and a command after -- are required')
    for field in ('heartbeat_seconds', 'notify_timeout'):
        value = getattr(args, field)
        if not math.isfinite(value) or value <= 0:
            parser.error(field.replace('_', '-') + ' must be finite and positive')
    cwd = Path(args.build_path).resolve()
    if not cwd.is_dir():
        parser.error('build path is not a directory')
    environment = {}
    for entry in args.env:
        key, separator, value = entry.partition('=')
        if not separator or not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', key):
            parser.error('--env requires NAME=VALUE')
        environment[key] = value
    screen = shutil.which('screen')
    if not screen:
        parser.error('screen must be available')
    # Read-only readiness check before launching any build or Screen session.
    if not check_channel(args.ipc_socket, args.thread, args.notify_timeout).get('ready'):
        raise RuntimeError('Direct chat channel unavailable')
    name = args.name or ('build-' + re.sub('[^A-Za-z0-9_-]', '', args.thread)[:12] + '-' + uuid.uuid4().hex[:10])
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,80}', name):
        parser.error('--name must contain 1-80 letters, digits, underscores or hyphens')
    existing = subprocess.run([screen, '-ls'], capture_output=True, text=True)
    if re.search(r'\d+\.' + re.escape(name) + r'\s', existing.stdout):
        parser.error('Screen session name already exists')
    state = Path(tempfile.mkdtemp(prefix='screen-build-' + name + '-', dir=args.state_root))
    config = dict(thread=args.thread, name=name, cwd=str(cwd), command=command,
                  environment=environment, heartbeat=args.heartbeat_seconds,
                  timeout=args.notify_timeout, socket=args.ipc_socket,
                  started_at=stamp())
    write_json(state / 'launch.json', config)
    write_json(state / 'status.json', dict(state='launching', name=name, thread=args.thread, started_at=config['started_at']))
    # Lowercase -dm forks and returns immediately, unlike uppercase -Dm.
    result = subprocess.run([screen, '-L', '-Logfile', str(state / 'screen.log'),
                             '-dmS', name, helper, '--run', str(state)],
                            stdin=subprocess.DEVNULL, capture_output=True, text=True)
    if result.returncode:
        write_json(state / 'status.json', dict(state='launch_failed', error=result.stderr))
        raise RuntimeError('Screen start failed: ' + result.stderr)
    # Verify helper startup, not build completion. Screen can otherwise report
    # success even when its command immediately fails (for example in a sandbox).
    deadline = time.monotonic() + 2
    while json.loads((state / 'status.json').read_text())['state'] == 'launching':
        if time.monotonic() >= deadline:
            raise RuntimeError('Helper did not acknowledge startup; inspect ' + str(state))
        time.sleep(0.02)
    print(json.dumps(dict(session=name, thread=args.thread, state_dir=str(state),
                          log=str(state / 'build.log'), status=str(state / 'status.json'))))


def run(state):
    config = json.loads((state / 'launch.json').read_text())
    child = None
    interrupted = None

    def handle_signal(number, frame):
        nonlocal interrupted
        interrupted = number
        if child is not None:
            try:
                os.killpg(child.pid, number)
            except ProcessLookupError:
                pass

    for number in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(number, handle_signal)

    def notify(message):
        event = dict(at=stamp(), message=message, transport='desktop-ipc',
                     message_id=str(uuid.uuid4()))
        try:
            result = send_message(config['socket'], config['thread'],
                                  '[Screen ' + config['name'] + '] ' + message,
                                  config['timeout'], config['cwd'], event['message_id'])
            event.update(accepted=result.get('accepted') is True, output=result)
        except (OSError, ValueError, RuntimeError) as error:
            event.update(accepted=False, error=str(error))
        event['finished_at'] = stamp()
        with (state / 'notifications.jsonl').open('a') as log:
            log.write(json.dumps(event) + '\n')
        return event['accepted']

    started = time.monotonic()
    code = 1
    error = None
    try:
        with (state / 'build.log').open('ab', buffering=0) as log:
            if interrupted is not None:
                raise RuntimeError('Interrupted before build start')
            child = subprocess.Popen(config['command'], cwd=config['cwd'],
                                     env=dict(os.environ, **config['environment']),
                                     stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            write_json(state / 'status.json', dict(state='running', name=config['name'],
                       thread=config['thread'], pid=child.pid, started_at=config['started_at']))
            next_heartbeat = started + config['heartbeat']
            stop_deadline = None
            while child.poll() is None:
                now = time.monotonic()
                if interrupted is not None:
                    if stop_deadline is None:
                        stop_deadline = now + 5
                    elif now >= stop_deadline:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                elif now >= next_heartbeat:
                    next_heartbeat = now + config['heartbeat']
                    notify('alive')
                time.sleep(0.1)
            code = child.wait()
    except Exception as exception:
        error = str(exception)
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
    success = code == 0 and interrupted is None and error is None
    message = 'Build erfolgreich' if success else 'Build fehlgeschlagen'
    # Persist the outcome before notifying; delivery failure never becomes success.
    status = dict(state='completed', name=config['name'], thread=config['thread'],
                  exit_code=code, signal=interrupted, error=error, message=message,
                  finished_at=stamp(), elapsed_seconds=round(time.monotonic() - started, 3))
    write_json(state / 'status.json', status)
    status['notification_accepted'] = notify(message)
    write_json(state / 'status.json', status)
    # This is Screen's sole foreground command: returning closes the session.
    return 0 if success and status['notification_accepted'] else 1


try:
    if sys.argv[1:2] == ['--run'] and len(sys.argv) == 3:
        sys.exit(run(Path(sys.argv[2])))
    launch(sys.argv[2:] if sys.argv[1:2] == ['--launch'] else sys.argv[1:])
except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
    print('screen-build: ' + str(error), file=sys.stderr)
    sys.exit(1)
PY
