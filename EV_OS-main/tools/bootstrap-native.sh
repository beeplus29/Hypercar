#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPS="$ROOT/.deps"
CYCLONE_SRC="$DEPS/cyclonedds-src"
CYCLONE_BUILD="$DEPS/cyclonedds-build"
CYCLONE_PREFIX="$DEPS/cyclonedds"
CYCLONE_TAG="0.10.5"
CYCLONE_COMMIT="2cdd114cbd18340c606573b4cc8dc20cc161ec5a"
mkdir -p "$DEPS"

if command -v cmake >/dev/null 2>&1; then
    CMAKE="$(command -v cmake)"
elif [[ "$(uname -s)" == "Darwin" ]] && command -v python3 >/dev/null 2>&1; then
    python3 -m pip install --disable-pip-version-check --upgrade \
        --target "$DEPS/python-tools" "cmake==4.4.4"
    CMAKE="$DEPS/python-tools/cmake/data/bin/cmake"
else
    echo "CMake >= 3.28 is required. Install it with the host package manager, then rerun." >&2
    exit 1
fi

if [[ ! -x "$CMAKE" ]]; then
    echo "CMake executable is unavailable: $CMAKE" >&2
    exit 1
fi

if [[ ! -d "$CYCLONE_SRC/.git" ]]; then
    git clone --depth 1 --branch "$CYCLONE_TAG" \
        https://github.com/eclipse-cyclonedds/cyclonedds.git "$CYCLONE_SRC"
fi

ACTUAL_COMMIT="$(git -C "$CYCLONE_SRC" rev-parse HEAD)"
if [[ "$ACTUAL_COMMIT" != "$CYCLONE_COMMIT" ]]; then
    echo "Cyclone DDS source mismatch: expected $CYCLONE_COMMIT, found $ACTUAL_COMMIT" >&2
    echo "Refusing to replace existing source at $CYCLONE_SRC." >&2
    exit 1
fi

"$CMAKE" -S "$CYCLONE_SRC" -B "$CYCLONE_BUILD" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$CYCLONE_PREFIX" \
    -DBUILD_IDLC=ON \
    -DBUILD_DDSPERF=OFF \
    -DBUILD_TESTING=OFF \
    -DBUILD_EXAMPLES=OFF
"$CMAKE" --build "$CYCLONE_BUILD" --parallel 4
"$CMAKE" --install "$CYCLONE_BUILD"

"$CYCLONE_PREFIX/bin/idlc" -v
echo "Cyclone DDS installed locally at $CYCLONE_PREFIX"
