"""Direct, same-user Desktop IPC notifications; Python standard library only.

The versioned follower protocol is internal to the desktop app. Unsupported
versions, missing owners, and uncertain delivery fail visibly without queuing.
"""

import argparse
import json
import os
from pathlib import Path
import socket
import stat
import struct
import time
import uuid


def default_socket():
    return str(Path(os.environ.get('CODEX_HOME', Path.home() / '.codex')) / 'ipc' / 'ipc.sock')


class DesktopIPC:
    def __init__(self, endpoint, timeout):
        self.deadline = time.monotonic() + timeout
        self.client = 'initializing-client'
        path = Path(endpoint)
        parent, entry = path.parent.stat(), path.lstat()
        if (not stat.S_ISSOCK(entry.st_mode) or entry.st_uid != os.getuid()
                or parent.st_uid != os.getuid() or parent.st_mode & 0o022):
            raise RuntimeError('Desktop IPC socket must belong to the current user in a private directory')
        self.connection = socket.socket(socket.AF_UNIX)
        try:
            self.connection.settimeout(self.remaining())
            self.connection.connect(str(path))
            result = self.request('initialize', {'clientType': 'cli'}, version=0)
            self.client = result['result']['clientId']
        except BaseException:
            self.close()
            raise

    def remaining(self):
        value = self.deadline - time.monotonic()
        if value <= 0:
            raise TimeoutError('Desktop IPC notification timed out; delivery may be unknown')
        return value

    def close(self):
        self.connection.close()

    def write(self, message):
        payload = json.dumps(message).encode('utf-8')
        self.connection.settimeout(self.remaining())
        self.connection.sendall(struct.pack('<I', len(payload)) + payload)

    def read_exact(self, length):
        result = bytearray()
        while len(result) < length:
            self.connection.settimeout(self.remaining())
            chunk = self.connection.recv(length - len(result))
            if not chunk:
                raise RuntimeError('Desktop IPC connection closed; delivery may be unknown')
            result.extend(chunk)
        return bytes(result)

    def request(self, method, params, version=1, owner=None):
        request_id = str(uuid.uuid4())
        request = dict(type='request', requestId=request_id, sourceClientId=self.client,
                       version=version, method=method, params=params,
                       timeoutMs=max(1, int(self.remaining() * 1000)))
        if owner is not None:
            request['targetClientId'] = owner
        self.write(request)
        while True:
            length = struct.unpack('<I', self.read_exact(4))[0]
            if not 0 < length <= 16 * 1024 * 1024:
                raise RuntimeError('Invalid or oversized Desktop IPC response')
            response = json.loads(self.read_exact(length))
            if response.get('type') == 'client-discovery-request':
                # This client sends notifications; it owns no chats or requests.
                self.write(dict(type='client-discovery-response', requestId=response['requestId'],
                                response={'canHandle': False}))
            if response.get('type') == 'response' and response.get('requestId') == request_id:
                if response.get('resultType') != 'success':
                    raise RuntimeError(response.get('error', 'Desktop IPC request failed'))
                return response

    def owner(self, thread):
        result = self.request('thread-owner-discovery', dict(hostId='local', conversationId=thread))
        return result['handledByClientId']


def check_channel(endpoint, thread, timeout):
    ipc = DesktopIPC(endpoint, timeout)
    try:
        return dict(ready=True, transport='desktop-ipc', owner=ipc.owner(thread))
    finally:
        ipc.close()


def send_message(endpoint, thread, message, timeout, cwd, message_id=None):
    ipc = DesktopIPC(endpoint, timeout)
    try:
        owner = ipc.owner(thread)
        message_id = message_id or str(uuid.uuid4())
        inputs = [dict(type='text', text=message, text_elements=[])]
        restore = dict(id=message_id, text=message, cwd=cwd, createdAt=int(time.time() * 1000),
                       context=dict(prompt=message, addedFiles=[], fileAttachments=[],
                                    ideContext=None, imageAttachments=[], workspaceRoots=[cwd]))
        try:
            response = ipc.request('thread-follower-steer-turn', dict(
                conversationId=thread, clientUserMessageId=message_id, input=inputs,
                restoreMessage=restore, attachments=[]), owner=owner)
            delivery = 'steer'
        except RuntimeError as error:
            # Only this explicit pre-dispatch rejection permits starting a turn.
            # Timeouts, disconnects and arbitrary errors never replay a message.
            if str(error) != f'Cannot steer conversation {thread} because its active turn already ended':
                raise
            response = ipc.request('thread-follower-start-turn', dict(
                conversationId=thread, turnStart=dict(
                    request=dict(threadId=thread, input=inputs, clientUserMessageId=message_id),
                    context=dict(restoreMessage=restore, inheritThreadSettings=True))),
                version=2, owner=owner)
            delivery = 'start'
        result = response.get('result', {}).get('result', {})
        # Steer returns a turn ID; start returns the normal turn-start outcome.
        return dict(accepted=True, transport='desktop-ipc', delivery=delivery,
                    thread=thread, message_id=message_id, result=result)
    finally:
        ipc.close()


def main():
    parser = argparse.ArgumentParser(description='Send a direct Desktop IPC message; never queue it.')
    parser.add_argument('--thread', default=os.environ.get('CODEX_THREAD_ID'), required=False)
    parser.add_argument('--ipc-socket', default=default_socket())
    parser.add_argument('--timeout', type=float, default=20)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--message')
    parser.add_argument('--cwd', default=os.getcwd())
    args = parser.parse_args()
    if not args.thread or (not args.check and not args.message):
        parser.error('--thread and --message (or --check) are required')
    import math
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error('--timeout must be finite and positive')
    if args.check:
        result = check_channel(args.ipc_socket, args.thread, args.timeout)
    else:
        result = send_message(args.ipc_socket, args.thread, args.message, args.timeout, args.cwd)
    print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        raise SystemExit('screen-chat: ' + str(error))
