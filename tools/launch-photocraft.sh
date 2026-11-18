#!/bin/sh
# Starts a separate PhotoCraft with its control server on, for the scripts in this folder.
#
#   tools/launch-photocraft.sh [FILES…]
#
# Automation may read from target/photocraft-test and this project folder, and write only to
# target/photocraft-test (test images, exports, the control token). PHOTOCRAFT overrides the
# executable, PC_PORT the port (7878).
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
work="$here/target/photocraft-test"
mkdir -p "$work"
exec "${PHOTOCRAFT:-photocraft}" --new-instance --control "${PC_PORT:-7878}" \
    --control-token-file "$work/control.token" \
    --automation-read-root "$work" --automation-read-root "$here" \
    --automation-write-root "$work" "$@"
