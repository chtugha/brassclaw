"""Read one build observation using drained Modal exec pipes, without a PTY.

Modal 1.6.3's container CLI waits for process exit but starts output printing in
background tasks without awaiting their drain. Short commands can exit zero
with no captured output. This small version-checked adapter uses the identical
authenticated router and PIPE readers, and awaits stdout, stderr AND exit.
It cannot submit a Function, start Cargo, stop a container or select Python code.
"""

from __future__ import annotations

import argparse
import asyncio
from importlib.metadata import version
import json
import re
import sys
import uuid


SUPPORTED_MODAL_VERSION = "1.6.3"
INSPECTION_PROGRAM = (
    "import json, sys; sys.path.insert(0, '/opt/brassclaw'); "
    "from modal_build_worker import inspect_build; "
    "print(json.dumps(inspect_build('/build', json.loads(sys.argv[1]))))"
)


async def drain_process(process):
    """Exit acknowledgement alone does not prove either output pipe was read."""
    stdout, stderr, code = await asyncio.gather(process.stdout.read(), process.stderr.read(), process.wait())
    return stdout, stderr, code


async def _inspect(container_id, request):
    # No public Function-container exec handle exists in this SDK. Keep its
    # CLI-compatible private bridge here, behind an explicit tested version.
    from modal.client import _Client
    from modal.container_process import _ContainerProcess
    from modal.sandbox._sandbox import TaskCommandRouterClient, sr_pb2, v1_command_router_access_fetcher
    from modal.stream_type import StreamType

    client = await _Client.from_env()
    access = v1_command_router_access_fetcher(client, container_id)
    router = await TaskCommandRouterClient.connect(client, container_id, access)
    process_id = str(uuid.uuid4())
    await router.exec_start(sr_pb2.TaskExecStartRequest(
        task_id=container_id, exec_id=process_id,
        command_args=["/usr/local/bin/python3", "-c", INSPECTION_PROGRAM, json.dumps(request)],
        stdout_config=sr_pb2.TaskExecStdoutConfig.TASK_EXEC_STDOUT_CONFIG_PIPE,
        stderr_config=sr_pb2.TaskExecStderrConfig.TASK_EXEC_STDERR_CONFIG_PIPE,
    ))
    process = _ContainerProcess(process_id, container_id, client, command_router_client=router,
                                stdout=StreamType.PIPE, stderr=StreamType.PIPE)
    return await drain_process(process)


def inspect_container(container_id, request):
    if re.fullmatch(r"ta-[A-Za-z0-9_-]+", container_id) is None:
        raise ValueError("Expected an original Function-container ID")
    if not isinstance(request, dict) or request.get("environment") != "brassclaw":
        raise ValueError("Inspection requires the original brassclaw request")
    if str(uuid.UUID(request["job_id"])) != request["job_id"]:
        raise ValueError("Inspection requires the canonical original job UUID")
    if version("modal") != SUPPORTED_MODAL_VERSION:
        raise RuntimeError("Modal inspection transport requires validated SDK " + SUPPORTED_MODAL_VERSION)
    from modal._utils.async_utils import synchronizer

    # StreamReader uses Modal's own event loop; asyncio.run would use a different
    # loop and violate the SDK's stream ownership assertion.
    return synchronizer.create_blocking(_inspect)(container_id, request)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("container_id")
    parser.add_argument("request", help="Original admitted job JSON, always data")
    args = parser.parse_args()
    stdout, stderr, code = inspect_container(args.container_id, json.loads(args.request))
    sys.stdout.write(stdout)
    sys.stdout.flush()
    sys.stderr.write(stderr)
    sys.stderr.flush()
    return code


if __name__ == "__main__":
    raise SystemExit(main())
