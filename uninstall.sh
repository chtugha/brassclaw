#!/usr/bin/env bash
# Self-contained Linux/macOS BrassClaw uninstaller. Enter at the prompt wipes;
# type keep to retain state, or cancel to stop. No terminal means no implicit wipe.
# --dry-run inventories only; --yes / --wipe explicitly authorize unattended wipe.
# Python 3.9+ is required for safe path handling and ownership discovery.
set -euo pipefail
command -v python3 >/dev/null || { printf 'Python 3.9+ is required; nothing removed.\n' >&2; exit 1; }
python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)' || { printf 'Python 3.9+ is required; nothing removed.\n' >&2; exit 1; }
exec python3 - "$@" <<'PY'
import argparse
import fnmatch
import json
import os
from pathlib import Path
import plistlib
import pwd
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time

APP_NAMES = {'brassclaw', 'brassclaw-reborn', 'monty_worker'}
TEMP_PATTERNS = ('brassclaw-download.*', 'brassclaw-native-*', 'brassclaw-command-outputs',
                 'brassclaw-postgresql-bin-cache', 'brassclaw-http-broker.sock',
                 'brassclaw-secret-broker.sock', 'systemd-private-*-brassclaw.service-*')
ASSET_PATTERN = re.compile(r'brassclaw-(linux-amd64|macos-arm64|macos-amd64)(-monty-worker)?(\.sha256)?$')
CONTAINER_PATTERN = re.compile(r'brassclaw$|brassclaw-reborn-sandbox-[0-9a-f]{24}-[0-9a-f-]+$|brassclaw-sandbox-(install|run)-[0-9]+-[0-9]+$')
UNIT_PATTERN = re.compile(r'brassclaw(?:[-.][A-Za-z0-9_-]+)*\.(service|timer|socket|path)$')


def run(args, *, env=None, check=False, timeout=30):
    result = subprocess.run(args, env=env, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout)
    if check and result.returncode:
        raise RuntimeError('Command failed: ' + ' '.join(args[:3]))
    return result


def environment_file(path):
    """Read assignments as data; never source an operator file as shell code."""
    values = {}
    for line in Path(path).read_text().splitlines():
        if not line.strip() or line.lstrip().startswith(('#', ';')):
            continue
        parts = shlex.split(line, comments=True)
        if len(parts) == 1 and '=' in parts[0]:
            name, value = parts[0].split('=', 1)
            if re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', name):
                values[name] = value
        else:
            raise RuntimeError('Cannot safely parse environment file: ' + str(path))
    return values


def process_table():
    processes = {}
    if sys.platform.startswith('linux'):
        for directory in Path('/proc').iterdir():
            if not directory.name.isdigit():
                continue
            try:
                fields = (directory / 'stat').read_text().rsplit(')', 1)[1].split()
                exe = os.readlink(directory / 'exe').removesuffix(' (deleted)')
                env = dict(entry.split('=', 1) for entry in
                           (directory / 'environ').read_bytes().decode(errors='replace').split('\0')
                           if '=' in entry)
                processes[int(directory.name)] = (int(fields[1]), fields[19], Path(exe), env)
            except (OSError, ValueError):
                continue
    else:
        args = ['ps', '-axo', 'pid=,ppid=,lstart=,comm='] if os.geteuid() == 0 else ['ps', '-u', str(os.getuid()), '-o', 'pid=,ppid=,lstart=,comm=']
        for line in run(args).stdout.splitlines():
            fields = line.split(None, 7)
            if len(fields) == 8:
                exe = Path(fields[7])
                if exe.name in APP_NAMES | {'postgres', 'executable'} or ASSET_PATTERN.fullmatch(exe.name):
                    if not shutil.which('lsof'):
                        raise RuntimeError('lsof is required to verify running executable paths on macOS.')
                    names = run(['lsof', '-a', '-p', fields[0], '-d', 'txt', '-Fn']).stdout.splitlines()
                    matches = [Path(n[1:]) for n in names if n.startswith('n') and Path(n[1:]).name == exe.name]
                    if matches:
                        exe = matches[0]
                processes[int(fields[0])] = (int(fields[1]), ' '.join(fields[2:7]), exe, {})
    return processes


class Plan:
    def __init__(self, users=()):
        self.users = list(users)
        broad = ('/', '/etc', '/usr', '/usr/local', '/usr/local/bin', '/bin', '/sbin',
                 '/lib', '/var', '/var/lib', '/var/log', '/tmp', '/var/tmp', '/home',
                 '/Users', '/opt', '/srv', '/mnt', '/media', '/Volumes', '/Library')
        self.protected = {Path(p).resolve() for p in broad}
        self.protected.update(Path(u.pw_dir).resolve() for u in self.users)
        self.paths = {}
        self.roots = set()
        self.system_units = set()
        self.user_units = []
        self.launch_agents = []
        self.bin_dirs = set()
        self.envs = []
        self.issues = []
        self.packages = []
        self.containers = []
        self.images = []
        self.volumes = []
        self.locks = []
        self.brew_packages = []

    def validate(self, path):
        path = Path(path)
        if not path.is_absolute() or '..' in path.parts:
            raise ValueError('Refusing ambiguous deletion path: ' + str(path))
        # Resolve ancestors, but never follow a symlink at the deletion leaf.
        effective = path.parent.resolve() / path.name
        if effective.resolve() in self.protected or effective in self.protected:
            raise ValueError('Refusing a system directory or user home: ' + str(path))
        if not path.is_symlink() and path.exists() and os.path.ismount(path):
            raise ValueError('Refusing a filesystem mount root: ' + str(path))
        return effective

    def add(self, path, category, reason):
        path = Path(path)
        if not path.exists() and not path.is_symlink():
            return
        try:
            effective = self.validate(path)
        except ValueError as error:
            self.issues.append(str(error))
            return
        self.paths[path] = (category, reason, effective)

    def add_root(self, path, reason):
        path = Path(path)
        self.add(path, 'data', reason)
        if path in self.paths:
            self.roots.add(path.resolve())
            if path.is_symlink():
                target = path.resolve()
                # A link alone does not prove its destination belongs to the app.
                if (target / 'postgres/data/PG_VERSION').exists() or (target / 'config.toml').exists():
                    self.add(target, 'data', 'configured BrassClaw state behind a symlink')
                else:
                    self.issues.append('Symlink destination could not be qualified as BrassClaw state: ' + str(target))

    def consume_env(self, env):
        self.envs.append(env)
        for key in ('BRASSCLAW_REBORN_HOME', 'BRASSCLAW_BASE_DIR'):
            if env.get(key):
                candidate = Path(env[key])
                if candidate.exists() and not candidate.is_dir():
                    self.issues.append('Configured state root is not a directory: ' + str(candidate))
                else:
                    self.add_root(candidate, key)
        key_file = env.get('BRASSCLAW_SECRETS_PASSPHRASE_FILE')
        if key_file and Path(key_file).exists():
            key = Path(key_file)
            if any(root == key.parent.resolve() or root in key.parent.resolve().parents for root in self.roots):
                self.add(key, 'data', 'BrassClaw passphrase file')
            else:
                self.issues.append('Configured passphrase file outside owned state requires explicit cleanup: ' + str(key))
        if env.get('BRASSCLAW_PG_URL'):
            self.issues.append('An external PostgreSQL database is configured; its BrassClaw data must be reset separately. No shared database is dropped automatically.')

    def binaries(self, directory):
        directory = Path(directory)
        self.bin_dirs.add(directory.resolve())
        for name in APP_NAMES:
            for suffix in ('', '.bak', '.sha256'):
                self.add(directory / (name + suffix), 'runtime', 'application/worker executable or backup')
        if directory.is_dir():
            for path in directory.glob('.brassclaw-install.*'):
                if path.name == '.brassclaw-install.lock':
                    self.locks.append(path)
                else:
                    self.add(path, 'runtime', 'interrupted installer staging')
            for path in directory.iterdir():
                if ASSET_PATTERN.fullmatch(path.name):
                    self.add(path, 'runtime', 'downloaded release asset')

    def scan_temporary(self, directory):
        directory = Path(directory)
        if not directory.is_dir():
            return
        try:
            entries = list(directory.iterdir())
        except OSError:
            return
        for path in entries:
            if any(fnmatch.fnmatchcase(path.name, pattern) for pattern in TEMP_PATTERNS):
                self.add(path, 'runtime', 'BrassClaw temporary runtime/download artifact')
            elif ASSET_PATTERN.fullmatch(path.name):
                self.add(path, 'runtime', 'downloaded release asset')
            elif path.is_dir() and not path.is_symlink() and path.name.startswith('tmp'):
                # Older installers used unbranded mktemp directories. Only a
                # directory containing exclusively known release assets qualifies.
                try:
                    children = list(path.iterdir())
                except OSError:
                    continue
                if children and all(p.is_file() and not p.is_symlink() and ASSET_PATTERN.fullmatch(p.name) for p in children):
                    self.add(path, 'runtime', 'legacy installer download directory')

    def discover_service(self):
        if os.geteuid() != 0 or not shutil.which('systemctl') or not Path('/run/systemd/system').exists():
            return
        roots = ('/etc/systemd/system', '/run/systemd/system', '/usr/lib/systemd/system', '/lib/systemd/system')
        self.system_units.add('brassclaw.service')
        for directory in roots:
            for path in Path(directory).glob('brassclaw*'):
                name = path.name.removesuffix('.d')
                if UNIT_PATTERN.fullmatch(name):
                    self.system_units.add(name)
                    self.add(path, 'runtime', 'systemd unit/drop-in')
            for path in Path(directory).glob('*.wants/brassclaw*'):
                if UNIT_PATTERN.fullmatch(path.name):
                    self.add(path, 'runtime', 'systemd enablement link')
        for unit in sorted(self.system_units):
            result = run(['systemctl', 'cat', unit])
            if result.returncode:
                continue
            environment = {}
            env_files = []
            for line in result.stdout.splitlines():
                if line.startswith('Environment='):
                    raw = line.split('=', 1)[1]
                    if not raw:
                        environment.clear()
                    for assignment in shlex.split(raw):
                        if '=' in assignment:
                            key, value = assignment.split('=', 1)
                            environment[key] = value
                elif line.startswith('EnvironmentFile='):
                    raw = line.split('=', 1)[1]
                    if not raw:
                        env_files.clear()
                    env_files.extend(part.lstrip('-') for part in shlex.split(raw))
            for file in env_files:
                if '%' in file:
                    self.issues.append('Unresolved systemd EnvironmentFile specifier: ' + file)
                    continue
                if Path(file).is_file():
                    environment.update(environment_file(file))
                    # Arbitrary env files may also configure other programs.
                    if 'brassclaw' in Path(file).parts or Path(file).name.startswith('brassclaw'):
                        self.add(file, 'data', 'BrassClaw environment/credentials')
                    else:
                        self.issues.append('Shared/custom environment file requires BrassClaw assignments to be removed: ' + file)
            self.consume_env(environment)

    def discover(self):
        for user in self.users:
            home = Path(user.pw_dir)
            if not home.is_dir():
                continue
            for path in (home / '.brassclaw', home / '.brassclaw-reborn',
                         home / '.config/brassclaw', home / '.cache/brassclaw',
                         home / '.local/share/brassclaw', home / 'Library/Application Support/BrassClaw',
                         home / 'Library/Caches/BrassClaw', home / 'Library/Logs/BrassClaw'):
                self.add_root(path, 'BrassClaw user state/cache/logs')
            for directory in (home / '.local/bin', home / 'bin', home / '.cargo/bin'):
                self.binaries(directory)
            self.scan_temporary(home / 'Downloads')
            self.scan_temporary(home / '.cache')
            for directory in (home / 'Library/Caches/Homebrew/downloads', home / '.cache/Homebrew/downloads'):
                for path in directory.glob('*--brassclaw-*'):
                    self.add(path, 'runtime', 'BrassClaw Homebrew package download')
            for path in (home / 'Library/Logs/DiagnosticReports').glob('brassclaw*'):
                self.add(path, 'runtime', 'BrassClaw crash report')
            for path in (home / 'Library/Logs/DiagnosticReports').glob('monty_worker*'):
                self.add(path, 'runtime', 'Monty worker crash report')
            for path in (home / '.config/systemd/user').glob('brassclaw*'):
                name = path.name.removesuffix('.d')
                if UNIT_PATTERN.fullmatch(name):
                    self.user_units.append((user, name))
                    self.add(path, 'runtime', 'user systemd unit/drop-in')
            for path in (home / '.config/systemd/user').glob('*.wants/brassclaw*'):
                if UNIT_PATTERN.fullmatch(path.name):
                    self.add(path, 'runtime', 'user systemd enablement link')
            for path in (home / 'Library/LaunchAgents').glob('*brassclaw*.plist'):
                self.launch_agents.append((user.pw_uid, path))
                self.add(path, 'runtime', 'BrassClaw launch agent')
        if os.geteuid() == 0:
            for path in ('/etc/brassclaw', '/var/lib/brassclaw', '/var/cache/brassclaw',
                         '/var/log/brassclaw', '/var/log/brassclaw.log', '/run/brassclaw',
                         '/opt/brassclaw', '/usr/local/lib/brassclaw', '/usr/local/share/brassclaw'):
                self.add_root(path, 'system BrassClaw installation/state')
            for path in Path('/var/lib/systemd/coredump').glob('core.brassclaw*'):
                self.add(path, 'runtime', 'BrassClaw core dump')
            for path in Path('/var/lib/systemd/coredump').glob('core.monty_worker*'):
                self.add(path, 'runtime', 'Monty worker core dump')
            for path in Path('/var/crash').glob('*brassclaw*'):
                self.add(path, 'runtime', 'BrassClaw crash report')
            for directory in ('/usr/local/bin', '/usr/bin', '/opt/homebrew/bin'):
                self.binaries(directory)
            for path in Path('/Library/LaunchDaemons').glob('*brassclaw*.plist'):
                self.launch_agents.append((None, path))
                self.add(path, 'runtime', 'BrassClaw launch daemon')
        for directory in {'/tmp', '/var/tmp', os.environ.get('TMPDIR', '/tmp')}:
            self.scan_temporary(directory)
        if sys.platform == 'darwin' and os.geteuid() == 0:
            for path in Path('/private/var/folders').glob('*/*/[TC]'):
                self.scan_temporary(path)
        self.consume_env(os.environ)
        for process in self.owned_processes().values():
            self.consume_env(process[3])
        self.discover_service()
        self.discover_packages_and_containers()
        self.discover_postgres_sockets()

    def discover_postgres_sockets(self):
        # PostgreSQL's lock identifies the cluster. A port alone never does.
        for root in self.roots:
            for relative in ('postgres/data', 'reborn/postgres/data', '.brassclaw/reborn/postgres/data'):
                data = root / relative
                pid_file = data / 'postmaster.pid'
                if not pid_file.is_file():
                    continue
                fields = pid_file.read_text().splitlines()
                if len(fields) < 5 or not fields[0].isdigit() or not fields[3].isdigit():
                    continue
                if Path(fields[1]).resolve() != data.resolve() or not Path(fields[4]).is_absolute():
                    continue
                socket = Path(fields[4]) / ('.s.PGSQL.' + fields[3])
                lock = Path(str(socket) + '.lock')
                if lock.is_file() and lock.read_text().splitlines()[:3] == fields[:3]:
                    self.add(socket, 'runtime', 'embedded PostgreSQL socket qualified by cluster lock')
                    self.add(lock, 'runtime', 'embedded PostgreSQL cluster socket lock')

    def owned_processes(self):
        table = process_table()
        owned = set()
        for pid, (_, _, exe, env) in table.items():
            if pid == os.getpid():
                continue
            parent = exe.parent.resolve()
            if exe.name in APP_NAMES and parent in self.bin_dirs:
                owned.add(pid)
            elif ASSET_PATTERN.fullmatch(exe.name) or (exe.name == 'executable' and parent.name.startswith('brassclaw-native-')):
                owned.add(pid)
            elif exe.name == 'postgres' and any(root in parent.parents for root in self.roots):
                owned.add(pid)
        while True:
            children = {pid for pid, info in table.items() if info[0] in owned}
            if children <= owned:
                break
            owned.update(children)
        return {pid: table[pid] for pid in owned}

    def discover_packages_and_containers(self):
        for executable in ('/opt/homebrew/bin/brew', '/usr/local/bin/brew'):
            if not Path(executable).is_file():
                continue
            owner = pwd.getpwuid(Path(executable).resolve().stat().st_uid)
            prefix = ['sudo', '-u', owner.pw_name, '--'] if os.geteuid() == 0 else []
            result = run(prefix + [executable, 'list', '--formula', '-1'])
            if result.returncode:
                self.issues.append('Homebrew packages could not be inventoried: ' + executable)
                continue
            for name in result.stdout.splitlines():
                if name in ('brassclaw', 'brassclaw-reborn'):
                    self.brew_packages.append((prefix, executable, name))
                    package_root = run(prefix + [executable, '--prefix', name], check=True).stdout.strip()
                    self.binaries(Path(package_root) / 'bin')
        if os.geteuid() == 0 and shutil.which('dpkg-query'):
            for name in ('brassclaw', 'brassclaw-reborn'):
                result = run(['dpkg-query', '-W', '-f=${Status}', name])
                if result.returncode == 0 and result.stdout == 'install ok installed':
                    self.packages.append(('apt-get', name))
            for path in Path('/var/cache/apt/archives').glob('brassclaw*.deb'):
                self.add(path, 'runtime', 'BrassClaw package download')
        if os.geteuid() == 0 and shutil.which('rpm'):
            for name in ('brassclaw', 'brassclaw-reborn'):
                if run(['rpm', '-q', name]).returncode == 0:
                    self.packages.append(('rpm', name))
        if shutil.which('docker'):
            result = run(['docker', 'ps', '-a', '--format', '{{.ID}} {{.Names}}'])
            if result.returncode:
                self.issues.append('Docker is installed but its resources could not be inventoried.')
                return
            self.containers = [line.split()[0] for line in result.stdout.splitlines()
                               if len(line.split()) == 2 and CONTAINER_PATTERN.fullmatch(line.split()[1])]
            for container in self.containers:
                detail = json.loads(run(['docker', 'inspect', container], check=True).stdout)[0]
                for mount in detail.get('Mounts', []):
                    if mount.get('Type') == 'bind' and mount.get('Destination') in ('/home/brassclaw/.brassclaw', '/var/lib/brassclaw', '/brassclaw/state'):
                        self.add_root(mount['Source'], 'BrassClaw container state bind mount')
                    elif mount.get('Type') == 'volume':
                        name = mount.get('Name')
                        if name:
                            references = run(['docker', 'ps', '-a', '--filter', 'volume=' + name, '--format', '{{.ID}}'], check=True).stdout.splitlines()
                            if all(reference in self.containers for reference in references):
                                self.volumes.append(name)
                            else:
                                self.issues.append('BrassClaw container uses a shared Docker volume requiring separate cleanup: ' + name)
            result = run(['docker', 'image', 'ls', '--format', '{{.Repository}} {{.Tag}}'])
            self.images = sorted({line.split()[0] + ':' + line.split()[1] for line in result.stdout.splitlines()
                                  if len(line.split()) == 2 and line.split()[0] in
                                  ('nearaidev/brassclaw', 'ghcr.io/chtugha/brassclaw', 'brassclaw') and line.split()[1] != '<none>'})
            result = run(['docker', 'volume', 'ls', '--format', '{{.Name}}'])
            self.volumes = sorted(set(self.volumes + [name for name in result.stdout.splitlines() if name == 'brassclaw' or name.startswith('brassclaw-')]))

    def preflight(self, wipe):
        # A mounted directory below an owned root must not be traversed by rmtree.
        mounts = []
        if Path('/proc/self/mountinfo').exists():
            for line in Path('/proc/self/mountinfo').read_text().splitlines():
                mounts.append(Path(line.split()[4].replace('\\040', ' ').replace('\\011', '\t')))
        for path, (category, _, effective) in self.paths.items():
            if not wipe and category == 'data':
                continue
            if self.validate(path) != effective:
                raise RuntimeError('Deletion path changed since inventory: ' + str(path))
            if not path.is_symlink() and any(path.resolve() in mount.parents for mount in mounts):
                raise RuntimeError('Owned state contains a mounted filesystem; unmount it first: ' + str(path))
        if wipe and self.issues:
            raise RuntimeError('Full wipe has unresolved resources; see the inventory warnings above.')

    def stop(self):
        for prefix, executable, name in self.brew_packages:
            # A formula may have been installed without a background service.
            services = json.loads(run(prefix + [executable, 'services', 'list', '--json'], check=True).stdout)
            if any(service.get('name') == name and service.get('status') == 'started' for service in services):
                run(prefix + [executable, 'services', 'stop', name], check=True, timeout=60)
        for unit in sorted(self.system_units):
            load = run(['systemctl', 'show', unit, '--property=LoadState', '--value'])
            if load.returncode == 0 and load.stdout.strip() != 'not-found':
                run(['systemctl', 'stop', unit], check=True, timeout=180)
                run(['systemctl', 'disable', unit], check=True)
        for user, unit in set(self.user_units):
            env = os.environ.copy()
            env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=/run/user/{}/bus'.format(user.pw_uid)
            prefix = ['runuser', '-u', user.pw_name, '--'] if os.geteuid() == 0 else []
            if Path('/run/user/{}/bus'.format(user.pw_uid)).exists():
                run(prefix + ['systemctl', '--user', 'stop', unit], env=env, check=True)
                run(prefix + ['systemctl', '--user', 'disable', unit], env=env, check=True)
                run(prefix + ['systemctl', '--user', 'daemon-reload'], env=env, check=True)
        for uid, path in self.launch_agents:
            label = plistlib.loads(path.read_bytes()).get('Label')
            domain = 'system' if uid is None else 'gui/' + str(uid)
            if label and run(['launchctl', 'print', domain + '/' + label]).returncode == 0:
                run(['launchctl', 'bootout', domain, str(path)], check=True)
        for sig, wait in ((signal.SIGINT, 10), (signal.SIGTERM, 5), (signal.SIGKILL, 5)):
            owned = self.owned_processes()
            for pid, info in owned.items():
                # Fence PID reuse immediately before each signal.
                current = process_table().get(pid)
                if current and current[1:3] == info[1:3]:
                    try:
                        os.kill(pid, sig)
                    except ProcessLookupError:
                        pass
            deadline = time.monotonic() + wait
            while self.owned_processes() and time.monotonic() < deadline:
                time.sleep(0.1)
            if not self.owned_processes():
                break
        if self.owned_processes():
            raise RuntimeError('BrassClaw processes are still running; no files have been deleted.')

    def clear_keychains(self):
        for user in self.users:
            home = Path(user.pw_dir)
            prefix = ['sudo', '-u', user.pw_name, '--'] if os.geteuid() == 0 and user.pw_uid != 0 else []
            if sys.platform == 'darwin' and (home / 'Library/Keychains').is_dir():
                result = run(prefix + ['security', 'find-generic-password', '-s', 'brassclaw', '-a', 'master_key'])
                if result.returncode == 0:
                    run(prefix + ['security', 'delete-generic-password', '-s', 'brassclaw', '-a', 'master_key'], check=True)
                elif result.returncode != 44:
                    raise RuntimeError('Could not verify BrassClaw keychain removal for ' + user.pw_name)
            elif sys.platform.startswith('linux') and (home / '.local/share/keyrings').is_dir():
                if not shutil.which('secret-tool') or not Path('/run/user/{}/bus'.format(user.pw_uid)).exists():
                    raise RuntimeError('Cannot clear BrassClaw keyring entry for {}: run in that user session with secret-tool available.'.format(user.pw_name))
                env = os.environ.copy()
                env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=/run/user/{}/bus'.format(user.pw_uid)
                result = run(prefix + ['secret-tool', 'clear', 'service', 'brassclaw', 'account', 'master_key'], env=env)
                if result.returncode and result.stderr.strip():
                    raise RuntimeError('BrassClaw keyring cleanup failed for ' + user.pw_name)

    def delete(self, wipe):
        self.preflight(wipe)
        for prefix, executable, name in self.brew_packages:
            run(prefix + [executable, 'uninstall', '--force', name], check=True, timeout=180)
        for manager, name in self.packages:
            command = [manager, 'purge', '-y', name] if manager == 'apt-get' else [manager, '-e', name]
            run(command, check=True, timeout=180)
        for container in self.containers:
            run(['docker', 'rm', '-f', container], check=True)
        if wipe:
            for volume in self.volumes:
                run(['docker', 'volume', 'rm', volume], check=True)
        for image in self.images:
            run(['docker', 'image', 'rm', image], check=True)
        for path, (category, _, effective) in sorted(self.paths.items(), key=lambda item: len(item[0].parts), reverse=True):
            if not wipe and category == 'data':
                continue
            if not path.exists() and not path.is_symlink():
                continue
            if self.validate(path) != effective:
                raise RuntimeError('Path changed during removal: ' + str(path))
            if path.is_symlink() or not path.is_dir():
                path.unlink()
            else:
                shutil.rmtree(path)
            print('Removed:', path)
        for user, unit in set(self.user_units):
            if Path('/run/user/{}/bus'.format(user.pw_uid)).exists():
                env = os.environ.copy()
                env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=/run/user/{}/bus'.format(user.pw_uid)
                prefix = ['runuser', '-u', user.pw_name, '--'] if os.geteuid() == 0 else []
                run(prefix + ['systemctl', '--user', 'daemon-reload'], env=env, check=True)
        if self.system_units:
            run(['systemctl', 'daemon-reload'], check=True)
            for unit in self.system_units:
                run(['systemctl', 'reset-failed', unit])
        remaining = [str(p) for p, (kind, _, _) in self.paths.items()
                     if (wipe or kind != 'data') and (p.exists() or p.is_symlink())]
        if remaining:
            raise RuntimeError('Removal incomplete: ' + ', '.join(remaining))


def confirm(stream):
    print('Enter = permanently WIPE all listed BrassClaw data; keep = uninstall but retain data; cancel = stop: ', end='', flush=True)
    reply = stream.readline()
    if reply == '':
        raise RuntimeError('No confirmation received; nothing was removed.')
    reply = reply.strip().lower()
    if reply in ('', 'wipe', 'yes', 'y'):
        return True
    if reply == 'keep':
        return False
    raise RuntimeError('Cancelled; nothing was removed.')


def main():
    parser = argparse.ArgumentParser(description='Remove BrassClaw. Enter at the confirmation wipes all discovered local state.')
    parser.add_argument('-y', '--yes', '--wipe', action='store_true', dest='yes', help='explicit non-interactive full wipe')
    parser.add_argument('--keep-data', action='store_true', help='retain state and credentials')
    parser.add_argument('--dry-run', action='store_true', help='print inventory without stopping or deleting anything')
    args = parser.parse_args()
    users = pwd.getpwall() if os.geteuid() == 0 else [pwd.getpwuid(os.getuid())]
    users = [u for u in users if Path(u.pw_dir).is_dir() or u.pw_name == 'brassclaw']
    plan = Plan(users)
    # Only an installation-owned, non-login system account may have its home
    # treated as an app root. Never delete an operator's actual home/account.
    dedicated = [u for u in users if u.pw_name == 'brassclaw' and 0 < u.pw_uid < 1000
                 and Path(u.pw_dir) in (Path('/var/lib/brassclaw'), Path('/home/brassclaw'))
                 and Path(u.pw_shell).name in ('nologin', 'false')]
    for user in dedicated:
        plan.protected.discard(Path(user.pw_dir).resolve())
        plan.add_root(user.pw_dir, 'dedicated BrassClaw system-account home')
    plan.discover()
    print('BrassClaw removal inventory:')
    for path, (category, reason, _) in sorted(plan.paths.items(), key=lambda item: str(item[0])):
        print('  [{}] {} ({})'.format(category, path, reason))
    for manager, name in plan.packages:
        print('  Package:', manager, name)
    for _, executable, name in plan.brew_packages:
        print('  Homebrew package:', executable, name)
    for name in plan.containers + plan.images + plan.volumes:
        print('  Docker resource:', name)
    for issue in sorted(set(plan.issues)):
        print('  Unresolved:', issue, file=sys.stderr)
    if args.dry_run:
        return
    if args.yes or args.keep_data:
        wipe = not args.keep_data
    else:
        try:
            with open('/dev/tty', 'r') as tty:
                wipe = confirm(tty)
        except OSError:
            raise RuntimeError('No terminal available; use --yes to explicitly authorize a wipe, or --keep-data.')
    plan.preflight(wipe)
    acquired = []
    try:
        for directory in sorted(plan.bin_dirs):
            if not directory.is_dir() or not os.access(directory, os.W_OK):
                continue
            lock = directory / '.brassclaw-install.lock'
            if lock.exists():
                owner = lock / 'owner.pid'
                if not owner.exists():
                    raise RuntimeError('Unqualified installer lock remains; inspect before removal: ' + str(lock))
                pid = int(owner.read_text())
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    shutil.rmtree(lock)
                else:
                    raise RuntimeError('Installer/uninstaller is already running: ' + str(lock))
            lock.mkdir(mode=0o700)
            (lock / 'owner.pid').write_text(str(os.getpid()))
            acquired.append(lock)
        plan.stop()
        if wipe:
            plan.clear_keychains()
        plan.delete(wipe)
        if wipe:
            for user in dedicated:
                run(['userdel', user.pw_name], check=True)
        print('BrassClaw local installation and state removed.' if wipe else 'BrassClaw uninstalled; data and credentials retained.')
        print('Shared OS journal records, shared dependencies and external databases are not erased by local file cleanup.')
    finally:
        for lock in acquired:
            if lock.exists():
                (lock / 'owner.pid').unlink()
                lock.rmdir()


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print('Uninstall failed:', error, file=sys.stderr)
        sys.exit(1)
PY
