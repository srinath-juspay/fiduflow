#!/usr/bin/env bash
# Regenerate the Lean models of both pay-flow crates via Charon + Aeneas (pinned binaries
# from ~/source/aeneas-namma-cloud/.tools unless AENEAS_TOOLS is set), derive the proofs
# for the macro version from the hand-written ones, and build everything.
#
#   psp-flow      (hand-written)  -> psp-lean/PspFlow     checked by psp-lean/PspProofs
#   psp-flow-dsl  (flow! macro)   -> psp-lean/PspFlowDsl  checked by psp-lean/PspProofsDsl
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="${AENEAS_TOOLS:-$HOME/source/aeneas-namma-cloud/.tools}"
CHARON="${CHARON:-$TOOLS/charon/bin/charon}"
AENEAS="${AENEAS:-$TOOLS/aeneas/bin/aeneas}"

mkdir -p "$ROOT/build"
translate() {
  local crate_dir="$1" llbc="$2" subdir="$3"
  ( cd "$ROOT/$crate_dir" && "$CHARON" cargo --preset=aeneas --dest-file "$ROOT/build/$llbc" )
  rm -rf "$ROOT/psp-lean/$subdir"
  "$AENEAS" -backend lean -filter-trait-methods -dest "$ROOT/psp-lean" -subdir "$subdir" -split-files "$ROOT/build/$llbc"
}
translate psp-flow     psp_flow.llbc     PspFlow
translate psp-flow-dsl psp_flow_dsl.llbc PspFlowDsl
python3 "$ROOT/scripts/derive-dsl-proofs.py"
( cd "$ROOT/psp-lean" && lake build )
