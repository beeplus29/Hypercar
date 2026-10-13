#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CYCLONE_PREFIX="${CYCLONEDDS_HOME:-$ROOT/.deps/cyclonedds}"

if [[ ! -x "$CYCLONE_PREFIX/bin/idlc" ]]; then
    echo "Cyclone DDS idlc not found at $CYCLONE_PREFIX/bin/idlc." >&2
    echo "Run tools/bootstrap-native.sh first or set CYCLONEDDS_HOME." >&2
    exit 1
fi

if [[ -x "$ROOT/.deps/python-tools/cmake/data/bin/cmake" ]]; then
    PATH="$ROOT/.deps/python-tools/cmake/data/bin:$PATH"
fi
export PATH
export CYCLONEDDS_HOME="$CYCLONE_PREFIX"

case "$(uname -s)" in
    Darwin)
        export DYLD_LIBRARY_PATH="$CYCLONE_PREFIX/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
        ;;
    Linux)
        export LD_LIBRARY_PATH="$CYCLONE_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        ;;
esac

exec cargo "$@"
