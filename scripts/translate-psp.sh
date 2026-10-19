#!/usr/bin/env bash
# Regenerate psp-lean/PspFlow/*.lean from psp-flow/ via Charon + Aeneas (pinned binaries
# from ~/source/aeneas-namma-cloud/.tools unless AENEAS_TOOLS is set), then build the
# generated model and the proofs.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="${AENEAS_TOOLS:-$HOME/source/aeneas-namma-cloud/.tools}"
CHARON="${CHARON:-$TOOLS/charon/bin/charon}"
AENEAS="${AENEAS:-$TOOLS/aeneas/bin/aeneas}"

mkdir -p "$ROOT/build"
( cd "$ROOT/psp-flow" && "$CHARON" cargo --preset=aeneas --dest-file "$ROOT/build/psp_flow.llbc" )
rm -rf "$ROOT/psp-lean/PspFlow"
"$AENEAS" -backend lean -filter-trait-methods -dest "$ROOT/psp-lean" -subdir PspFlow -split-files "$ROOT/build/psp_flow.llbc"
( cd "$ROOT/psp-lean" && lake build )
