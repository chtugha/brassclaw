"""Versioned same-user Desktop IPC; deferred notifications, Python stdlib only.

Internal app protocol: incompatible or incomplete observations fail closed.
Check-and-send is intentionally best effort, not an atomic app-side operation.
"""
import argparse
from collections import deque
import copy
import datetime
import json
import math
import os
from pathlib import Path
import select
import socket
import stat
import struct
import tempfile
import threading
import time
import uuid

MAX_FRAME = 16 * 1024 * 1024


def default_socket():
    return str(Path(os.environ.get('CODEX_HOME', Path.home() / '.codex')) / 'ipc' / 'ipc.sock')


class Rejected(RuntimeError):
    """Well-formed RPC error; does not by itself prove absence of dispatch."""


class ProtocolError(RuntimeError):
    pass


class UnknownDelivery(RuntimeError):
    pass


class StateUnavailable(RuntimeError):
    pass


class DesktopIPC:
    def __init__(self, endpoint, timeout):
        self.deadline = time.monotonic() + timeout
        self.client = 'initializing-client'
        self.buffer = bytearray()
        self.on_event = lambda message: None
        path = Path(endpoint)
        parent, entry = path.parent.stat(), path.lstat()
        if (not stat.S_ISSOCK(entry.st_mode) or entry.st_uid != os.getuid()
                or parent.st_uid != os.getuid() or parent.st_mode & 0o022):
            raise RuntimeError('Desktop IPC socket must belong to the current user in a private directory')
        self.connection = socket.socket(socket.AF_UNIX)
        try:
            self.connection.settimeout(self.remaining())
            self.connection.connect(str(path))
            self.client = self.request('initialize', {'clientType': 'cli'}, version=0)['result']['clientId']
        except BaseException:
            self.close()
            raise

    def remaining(self):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('Desktop IPC operation timed out')
        return remaining

    def close(self):
        self.connection.close()

    def write(self, message):
        payload = json.dumps(message).encode('utf-8')
        if len(payload) > MAX_FRAME:
            raise ValueError('Oversized IPC request')
        self.connection.settimeout(self.remaining())
        self.connection.sendall(struct.pack('<I', len(payload)) + payload)

    def receive(self, timeout):
        end = time.monotonic() + timeout
        while True:
            if len(self.buffer) >= 4:
                length = struct.unpack('<I', self.buffer[:4])[0]
                if not 0 < length <= MAX_FRAME:
                    raise ValueError('Invalid or oversized Desktop IPC frame')
                if len(self.buffer) >= length + 4:
                    message = json.loads(self.buffer[4:length + 4])
                    del self.buffer[:length + 4]
                    if not isinstance(message, dict):
                        raise ValueError('Invalid Desktop IPC message')
                    return message
            remaining = max(0, end - time.monotonic())
            if not select.select([self.connection], [], [], remaining)[0]:
                return None
            chunk = self.connection.recv(65536)
            if not chunk:
                raise ConnectionError('Desktop IPC disconnected')
            self.buffer.extend(chunk)

    def handle(self, message):
        if message.get('type') == 'client-discovery-request':
            self.write(dict(type='client-discovery-response', requestId=message['requestId'],
                            response={'canHandle': False}))
        else:
            self.on_event(message)

    def request(self, method, params, version=1, owner=None):
        request_id = str(uuid.uuid4())
        request = dict(type='request', requestId=request_id, sourceClientId=self.client,
                       version=version, method=method, params=params,
                       timeoutMs=max(1, int(self.remaining() * 1000)))
        if owner is not None:
            request['targetClientId'] = owner
        self.write(request)
        while True:
            response = self.receive(self.remaining())
            if response is None:
                raise TimeoutError('Desktop IPC response timed out')
            if response.get('type') == 'response' and response.get('requestId') == request_id:
                result_type = response.get('resultType')
                if result_type not in ('success', 'error'):
                    raise ProtocolError('Unsupported or missing Desktop IPC resultType')
                if result_type == 'error':
                    error = response.get('error')
                    if not isinstance(error, str) or not error.strip():
                        raise ProtocolError('Malformed Desktop IPC error response')
                    if response.get('result') not in (None, {}):
                        raise ProtocolError('Conflicting Desktop IPC error/result response')
                    raise Rejected(error)
                if response.get('error') is not None:
                    raise ProtocolError('Conflicting Desktop IPC success/error response')
                return response
            self.handle(response)

    def owner(self, thread):
        owner = self.request('thread-owner-discovery', dict(hostId='local', conversationId=thread))['handledByClientId']
        if not isinstance(owner, str) or not owner:
            raise StateUnavailable('Missing chat owner')
        return owner


def check_channel(endpoint, thread, timeout):
    ipc = DesktopIPC(endpoint, timeout)
    try:
        return dict(ready=True, transport='desktop-ipc', owner=ipc.owner(thread))
    finally:
        ipc.close()


def deliver(ipc, owner, thread, message, cwd, message_id):
    """One mutation attempt; only explicit inactive rejection permits start fallback."""
    inputs = [dict(type='text', text=message, text_elements=[])]
    restore = dict(id=message_id, text=message, cwd=cwd, createdAt=int(time.time() * 1000),
                   context=dict(prompt=message, addedFiles=[], fileAttachments=[],
                                ideContext=None, imageAttachments=[], workspaceRoots=[cwd]))
    try:
        try:
            response = ipc.request('thread-follower-steer-turn', dict(
                conversationId=thread, clientUserMessageId=message_id, input=inputs,
                restoreMessage=restore, attachments=[]), owner=owner)
            mode = 'steer'
        except Rejected as error:
            if str(error) != f'Cannot steer conversation {thread} because its active turn already ended':
                raise
            response = ipc.request('thread-follower-start-turn', dict(
                conversationId=thread, turnStart=dict(
                    request=dict(threadId=thread, input=inputs, clientUserMessageId=message_id),
                    context=dict(restoreMessage=restore, inheritThreadSettings=True))), version=2, owner=owner)
            mode = 'start'
        result = response['result']['result']
        if not isinstance(result, dict):
            raise ValueError('Invalid delivery acknowledgement')
        turn_id = result.get('turnId') if mode == 'steer' else (
            result.get('turn', {}).get('id') if isinstance(result.get('turn'), dict) else None)
        if not isinstance(turn_id, str) or not turn_id.strip():
            raise ValueError('Missing acknowledged turn identity for '+mode)
        return dict(accepted=True, transport='desktop-ipc', delivery=mode,
                    thread=thread, message_id=message_id, result=result)
    except Exception as error:
        # RPC/forwarding errors can follow dispatch too. Only the exact valid
        # inactive-turn error handled above proves steer did not dispatch.
        raise UnknownDelivery(str(error)) from error


def send_message(endpoint, thread, message, timeout, cwd, message_id=None):
    """Low-level one-shot IPC operation. Normal supervision uses Dispatcher below."""
    ipc = DesktopIPC(endpoint, timeout)
    try:
        return deliver(ipc, ipc.owner(thread), thread, message, cwd, message_id or str(uuid.uuid4()))
    finally:
        ipc.close()


def patch_state(state, patch):
    if not isinstance(patch, dict) or 'path' not in patch or 'op' not in patch:
        raise StateUnavailable('Invalid stream patch')
    path, operation = patch['path'], patch['op']
    if not isinstance(path, list) or operation not in ('add', 'replace', 'remove'):
        raise StateUnavailable('Unsupported stream patch')
    if operation != 'remove' and 'value' not in patch:
        raise StateUnavailable('Missing patch value')
    if not path:
        if operation != 'replace' or not isinstance(patch['value'], dict):
            raise StateUnavailable('Unsupported root patch')
        return copy.deepcopy(patch['value'])
    parent = state
    for key in path[:-1]:
        if isinstance(parent, list):
            if type(key) is not int or not 0 <= key < len(parent):
                raise StateUnavailable('Invalid patch array path')
        elif isinstance(parent, dict):
            if not isinstance(key, str) or key not in parent:
                raise StateUnavailable('Invalid patch object path')
        else:
            raise StateUnavailable('Patch traverses a scalar')
        parent = parent[key]
    key = path[-1]
    if isinstance(parent, list):
        limit = len(parent) + (operation == 'add')
        if type(key) is not int or not 0 <= key < limit:
            raise StateUnavailable('Invalid patch array index')
    elif isinstance(parent, dict):
        if not isinstance(key, str) or (operation != 'add' and key not in parent):
            raise StateUnavailable('Invalid patch object key')
    else:
        raise StateUnavailable('Patch target is a scalar')
    if operation == 'remove':
        if isinstance(parent, list):
            parent.pop(key)
        else:
            del parent[key]
    elif isinstance(parent, list) and operation == 'add':
        parent.insert(key, copy.deepcopy(patch['value']))
    else:
        parent[key] = copy.deepcopy(patch['value'])
    return state


def completion_time(item):
    value = item.get('completedAtMs')
    if value is not None:
        try:
            valid = type(value) in (int, float) and value >= 0 and math.isfinite(value)
        except OverflowError:
            valid = False
        if not valid:
            raise StateUnavailable('Invalid completion timestamp')
    return value


def tool_state(state):
    """App-visible calls only. A detached subprocess may outlive its tool item."""
    if not isinstance(state, dict) or not isinstance(state.get('turns'), list):
        raise StateUnavailable('Missing complete conversation state')
    turns = list(state['turns'])
    history = state.get('turnHistory')
    if history is not None:
        if history.get('kind') != 'canonical':
            raise StateUnavailable('Unsupported history layout')
        turns.extend(history['history']['entitiesByKey'].values())
    runtime = state.get('threadRuntimeStatus')
    if runtime is not None:
        if not isinstance(runtime, dict) or runtime.get('type') not in ('active', 'idle', 'notLoaded', 'systemError'):
            raise StateUnavailable('Unknown thread runtime state')
        if runtime['type'] in ('notLoaded', 'systemError'):
            raise StateUnavailable('Thread runtime is unavailable')
        if runtime['type'] == 'active' and (state.get('resumeState') == 'needs_resume' or
                not any(isinstance(turn, dict) and turn.get('status') == 'inProgress' for turn in turns)):
            raise StateUnavailable('Active runtime without loaded active turn')
    busy = []
    for turn in turns:
        status = turn.get('status')
        if status not in ('inProgress', 'completed', 'interrupted', 'failed'):
            raise StateUnavailable('Unknown turn status')
        if status != 'inProgress':
            continue
        if not isinstance(turn.get('items'), list):
            raise StateUnavailable('Missing active turn items')
        for item in turn['items']:
            if item.get('type') in ('reasoning', 'agentMessage', 'userMessage', 'steeringUserMessage', 'steered', 'plan'):
                continue
            completion = completion_time(item)
            status = item.get('status')
            if status is not None and status not in ('inProgress', 'running', 'pending', 'completed',
                                                       'failed', 'declined', 'interrupted', 'cancelled'):
                raise StateUnavailable('Unknown tool status')
            if status in ('inProgress', 'running', 'pending'):
                if completion is not None:
                    raise StateUnavailable('Contradictory tool status')
                busy.append(item.get('id'))
            elif status in ('completed', 'failed', 'declined', 'interrupted', 'cancelled'):
                continue
            elif 'completedAtMs' in item:
                if completion is None:
                    busy.append(item.get('id'))
            else:
                raise StateUnavailable('Unrecognized tool lifecycle: '+str(item.get('type'))+' keys='+','.join(sorted(item)))
    for request in state.get('requests', []):
        if completion_time(request) is None:
            busy.append('pending-request')
    for submission in state.get('unconfirmedTurnSubmissions', []):
        if submission.get('terminal') is not True:
            busy.append('pending-turn-submission')
    return busy


class Follower:
    def __init__(self, endpoint, thread, timeout):
        self.ipc = DesktopIPC(endpoint, timeout)
        self.thread = thread
        self.state = None
        self.revision = None
        self.ready = False
        self.activity = 0
        try:
            self.owner = self.ipc.owner(thread)
            self.ipc.on_event = self.event
            self.following(True)
        except BaseException:
            self.ipc.close()
            raise

    def following(self, value):
        self.ipc.write(dict(type='broadcast', sourceClientId=self.ipc.client, version=1,
                            method='thread-stream-following-changed', targetClientIds=[self.owner],
                            params=dict(hostId='local', conversationId=self.thread, following=value)))

    def event(self, message):
        method = message.get('method')
        if method == 'ipc-connection-reset':
            self.ready = False
            raise StateUnavailable('IPC connection reset')
        if method != 'thread-stream-state-changed':
            return
        params = message.get('params', {})
        if params.get('conversationId') != self.thread or params.get('hostId') != 'local':
            return
        if message.get('sourceClientId') != self.owner or message.get('version') != 13:
            self.ready = False
            raise StateUnavailable('Owner or stream protocol changed')
        self.ready = False
        change = params['change']
        revision = change['revision']
        if isinstance(revision, bool) or not isinstance(revision, int) or revision < 0:
            raise StateUnavailable('Invalid stream revision')
        if change['type'] == 'snapshot':
            if self.revision is not None and revision < self.revision:
                raise StateUnavailable('Regressing stream snapshot')
            self.state = change['conversationState']
        elif (change['type'] == 'patches' and self.state is not None
              and type(change.get('baseRevision')) is int
              and change['baseRevision'] == self.revision and revision > self.revision):
            for patch in change['patches']:
                self.state = patch_state(self.state, patch)
        else:
            raise StateUnavailable('Missing snapshot or stream revision gap')
        busy = tool_state(self.state)  # Unknown schema cannot grant a send decision.
        if busy:
            self.activity += 1
        self.revision = revision
        self.ready = True

    def poll(self, timeout):
        message = self.ipc.receive(timeout)
        if message is not None:
            self.ipc.handle(message)
        return self.ready

    def drain(self):
        # Bound work to avoid starvation under continuous output.
        for _ in range(256):
            message = self.ipc.receive(0)
            if message is None:
                return not self.ipc.buffer
            self.ipc.handle(message)
        return False

    def close(self):
        try:
            self.ipc.deadline = time.monotonic() + .2
            self.following(False)
        except Exception:
            pass  # Socket closure also removes the follower; never grants readiness.
        self.ipc.close()


class Dispatcher:
    """One bounded pending slot and one sender thread within the Screen helper."""
    def __init__(self, config, state_dir):
        self.config = config
        self.state_dir = Path(state_dir)
        self.lock = threading.Lock()
        self.wake = threading.Event()
        self.done = threading.Event()
        self.pending = None
        self.inflight = None
        self.view = dict(state='idle', accepted=None)
        self.log_error = None
        self.log_events = deque(maxlen=256)
        self.worker_error = None
        self.worker_exited = threading.Event()
        self.stop = False
        self.thread = threading.Thread(target=self._run, name='screen-notify', daemon=False)
        self.thread.start()

    def _log(self, event, item=None, **details):
        now = time.time()
        timestamp = datetime.datetime.fromtimestamp(now, datetime.timezone.utc).isoformat()
        entry = dict(at=timestamp, at_unix=now, event=event, transport='desktop-ipc', **details)
        if item:
            prefix = '[Screen ' + self.config.get('name', '') + '] '
            logical_message = item['message'][len(prefix):] if item['message'].startswith(prefix) else item['message']
            entry.update(message=logical_message, text=item['message'], message_id=item['id'], kind=item['kind'])
        if event in ('accepted', 'not_delivered', 'unknown'):
            entry['finished_at'] = timestamp
        # Callers hold the state lock. Never perform filesystem I/O under it.
        if len(self.log_events) == self.log_events.maxlen:
            self.log_error = 'Notification evidence buffer overflow; some records lost'
        self.log_events.append(entry)

    def _flush_log(self):
        """The worker writes evidence; after its join, close() drains any remainder."""
        with self.lock:
            events = list(self.log_events)
            self.log_events.clear()
        if not events:
            return
        fd = None
        try:
            flags = os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NONBLOCK | os.O_NOFOLLOW
            fd = os.open(self.state_dir / 'notifications.jsonl', flags, 0o600)
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
                raise OSError('Notification log must be a regular file owned by the current user')
            data = ''.join(json.dumps(entry)+'\n' for entry in events).encode('utf-8')
            while data:
                written = os.write(fd, data)
                if written <= 0:
                    raise OSError('Notification log write made no progress')
                data = data[written:]
        except Exception as error:
            with self.lock:
                self.log_error = str(error)
        finally:
            if fd is not None:
                try:
                    os.close(fd)
                except OSError as error:
                    with self.lock:
                        self.log_error = str(error)

    def submit(self, message, final=False, finished=None):
        with self.lock:
            if self.stop or self.done.is_set() or (self.pending and self.pending['kind'] == 'final') or (self.inflight and self.inflight['kind'] == 'final'):
                raise RuntimeError('Notification dispatcher already finalized')
            if self.worker_error is not None:
                # A notification worker failure must never become a build exception.
                item = dict(id=str(uuid.uuid4()), message=message, kind='final' if final else 'alive')
                self.view = dict(state='not_delivered', accepted=False, kind=item['kind'],
                                 message_id=item['id'], error='Notification worker unavailable: '+self.worker_error)
                self._log('not_delivered', item, accepted=False, error=self.view['error'])
                if final:
                    self.done.set()
                return False
            if self.pending:
                self._log('superseded', self.pending, accepted=None)
            self.pending = dict(id=str(uuid.uuid4()), message=message,
                                kind='final' if final else 'alive',
                                deadline=((finished if finished is not None else time.monotonic()) + self.config.get('final_wait', 600)) if final else None)
            self.view = dict(state='waiting_state', accepted=None, message_id=self.pending['id'], kind=self.pending['kind'])
            self._log('created', self.pending, accepted=None)
            self.wake.set()

    def snapshot(self):
        with self.lock:
            result = dict(self.view)
            if self.log_error:
                result['logging_error'] = self.log_error
            if self.worker_error:
                result['worker_error'] = self.worker_error
            return result

    def _view(self, item, state, **details):
        with self.lock:
            if self.pending is item or self.inflight is item:
                # An old in-flight alive must not overwrite a newer final's status.
                if self.pending is not None and self.pending is not item:
                    return
                changed = self.view.get('state') != state
                self.view = dict(state=state, accepted=None, message_id=item['id'], kind=item['kind'], **details)
                if changed:
                    self._log(state, item, **details)

    def _finish(self, item, outcome, accepted, **details):
        with self.lock:
            self._finish_locked(item, outcome, accepted, **details)

    def _finish_locked(self, item, outcome, accepted, **details):
        self._log(outcome, item, accepted=accepted, **details)
        if self.inflight is item:
            self.inflight = None
        if self.pending is item:
            self.pending = None
        if self.pending is None:
            self.view = dict(state=outcome, accepted=accepted, kind=item['kind'], message_id=item['id'], **details)
        if item['kind'] == 'final':
            self.done.set()

    def _run(self):
        follower = None
        idle_since = None
        reconnect_at = 0
        backoff = .2
        connected_at = 0
        owner_checked_at = 0
        try:
            while not self.done.is_set():
                self._flush_log()
                with self.lock:
                    if self.stop:
                        return
                    item = self.pending
                now = time.monotonic()
                if item and item['deadline'] is not None and now >= item['deadline']:
                    self._finish(item, 'not_delivered', False, error='Final notification deadline expired')
                    continue
                if item is None:
                    self.wake.wait(.05)
                    self.wake.clear()
                    continue
                try:
                    remaining = float('inf') if item['deadline'] is None else item['deadline'] - now
                    operation_timeout = min(self.config.get('timeout', 20), remaining)
                    if follower is None:
                        if now < reconnect_at:
                            self.wake.wait(min(.05, reconnect_at - now))
                            self.wake.clear()
                            continue
                        follower = Follower(self.config['socket'], self.config['thread'], min(operation_timeout, 1))
                        connected_at = time.monotonic()
                        owner_checked_at = connected_at
                    follower.ipc.deadline = time.monotonic() + min(self.config.get('timeout',20), max(.001,remaining))
                    activity = follower.activity
                    if time.monotonic()-owner_checked_at >= 1:
                        follower.ipc.deadline = time.monotonic()+min(1,operation_timeout)
                        if follower.ipc.owner(self.config['thread']) != follower.owner:
                            raise StateUnavailable('Chat owner changed')
                        owner_checked_at = time.monotonic()
                    follower.poll(min(.05, max(.001,remaining)))
                    if not follower.ready:
                        self._view(item, 'waiting_state')
                        if time.monotonic() - connected_at > self.config.get('timeout',20):
                            raise StateUnavailable('Snapshot timeout')
                        continue
                    if not follower.drain():
                        idle_since = None
                        continue
                    if follower.activity != activity:
                        idle_since = None
                    busy = tool_state(follower.state)
                    if busy:
                        idle_since = None
                        self._view(item, 'waiting_tool', busy_count=len(busy))
                        continue
                    idle_since = idle_since or time.monotonic()
                    self._view(item, 'waiting_quiet')
                    if time.monotonic() - idle_since < self.config.get('quiet', .3):
                        continue
                    # Rediscover the owner and process broadcasts received during that query.
                    follower.ipc.deadline = time.monotonic() + min(1, self.config.get('timeout',20), max(.001,remaining))
                    activity = follower.activity
                    if follower.ipc.owner(self.config['thread']) != follower.owner:
                        raise StateUnavailable('Chat owner changed')
                    if not follower.drain() or not follower.ready or tool_state(follower.state) or follower.activity != activity:
                        idle_since = None
                        continue
                    with self.lock:
                        if self.pending is not item:
                            continue
                        if item['deadline'] is not None and time.monotonic() >= item['deadline']:
                            continue
                        self.pending = None
                        self.inflight = item
                    self._view(item, 'sending')
                    remaining = self.config.get('timeout',20)
                    if item['deadline'] is not None:
                        remaining = min(remaining, item['deadline']-time.monotonic())
                    if remaining <= 0:
                        self._finish(item, 'not_delivered', False, error='Final notification deadline expired before send')
                        continue
                    follower.ipc.deadline = time.monotonic()+remaining
                    try:
                        # The send decision is over. Stream validation during a
                        # mutation must not hide an otherwise valid response.
                        follower.ipc.on_event = lambda message: None
                        output = deliver(follower.ipc, follower.owner, self.config['thread'],
                                         item['message'], self.config['cwd'], item['id'])
                    except Rejected as error:
                        self._finish(item, 'unknown', None, error=str(error))
                    except UnknownDelivery as error:
                        self._finish(item, 'unknown', None, error=str(error))
                    else:
                        self._finish(item, 'accepted', True, output=output)
                    # Do not reuse observations spanning a mutation/ambiguous outcome.
                    follower.close()
                    follower = None
                    idle_since = None
                    backoff = .2
                except Exception as error:
                    idle_since = None
                    with self.lock:
                        sending = self.inflight is item
                    if sending:
                        self._finish(item, 'unknown', None, error=str(error))
                    else:
                        self._view(item, 'waiting_state', error=str(error))
                        with self.lock:
                            self._log('observation_error', item, error=str(error))
                    if follower:
                        follower.close()
                    follower = None
                    reconnect_at = time.monotonic() + backoff
                    backoff = min(5, backoff*2)
        except BaseException as error:
            with self.lock:
                self.worker_error = str(error) or type(error).__name__
                inflight, pending = self.inflight, self.pending
                # Failure publication and slot finalization are one state change;
                # a concurrently submitted final must never be overwritten by alive.
                if inflight:
                    self._finish_locked(inflight, 'unknown', None, error='Notifier failed: '+self.worker_error)
                if pending:
                    self._finish_locked(pending, 'not_delivered', False, error='Notifier failed: '+self.worker_error)
        finally:
            try:
                if follower:
                    follower.close()
                self._flush_log()
            finally:
                self.worker_exited.set()

    def close(self):
        with self.lock:
            self.stop = True
        self.wake.set()
        self.thread.join()  # All socket operations have bounded deadlines.
        self._flush_log()


def main():
    parser = argparse.ArgumentParser(description='Deferred direct Desktop IPC; never a chat queue. Run message mode inside Screen to avoid observing your own foreground command.')
    parser.add_argument('--thread', default=os.environ.get('CODEX_THREAD_ID'))
    parser.add_argument('--ipc-socket', default=default_socket())
    parser.add_argument('--timeout', type=float, default=20)
    parser.add_argument('--quiet-seconds', type=float, default=.3)
    parser.add_argument('--final-wait-seconds', type=float, default=600)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--message')
    parser.add_argument('--state-dir')
    parser.add_argument('--cwd', default=os.getcwd())
    args = parser.parse_args()
    if not args.thread or (not args.check and not args.message):
        parser.error('--thread and --message (or --check) are required')
    for name in ('timeout','quiet_seconds','final_wait_seconds'):
        if not math.isfinite(getattr(args,name)) or getattr(args,name)<=0:
            parser.error(name+' must be finite and positive')
    if args.check:
        print(json.dumps(check_channel(args.ipc_socket,args.thread,args.timeout)))
        return
    state_dir = args.state_dir or tempfile.mkdtemp(prefix='screen-chat-notify-')
    if not Path(state_dir).is_dir():
        parser.error('--state-dir must be an existing directory')
    dispatcher = Dispatcher(dict(socket=args.ipc_socket,thread=args.thread,cwd=args.cwd,
                                 timeout=args.timeout,quiet=args.quiet_seconds,final_wait=args.final_wait_seconds),state_dir)
    try:
        dispatcher.submit(args.message,final=True)
        dispatcher.done.wait()
    finally:
        dispatcher.close()
    result=dispatcher.snapshot()
    result['state_dir'] = str(state_dir)
    print(json.dumps(result))
    if result.get('accepted') is not True:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
