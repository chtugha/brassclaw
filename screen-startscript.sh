#!/bin/bash
# Start detached with direct Desktop IPC notifications (no Node or queue).
# All options/command arguments are passed unchanged to the helper.
# Prints launch JSON and exits; the helper and notifications run inside Screen.
# See ./screen-startscript.sh --help for usage.
set -euo pipefail
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$script_dir/screen-build-helper.sh" --launch "$@"
