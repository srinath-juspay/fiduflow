#!/usr/bin/env python3
"""Derive psp-lean/PspProofsDsl/Pay.lean from psp-lean/PspProofs/Pay.lean.

The macro's expansion differs from the hand-written psp-flow in exactly three ways that the
proofs can see: the user-notification predicate is not in the code (so it is defined in the
proof), the decisions live in the `machine` module, and `start` takes the entity handle.
Everything else, including every theorem and every proof script, is identical.
"""
import os, re, sys

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src_path = os.path.join(root, "psp-lean", "PspProofs", "Pay.lean")
dst_dir = os.path.join(root, "psp-lean", "PspProofsDsl")
s = open(src_path).read()

s = s.replace("import PspFlow\n", "import PspFlowDsl\n")
s = s.replace("namespace psp_flow\n", "namespace psp_flow_dsl\n").replace("end psp_flow\n", "end psp_flow_dsl\n")
s = s.replace(
    """Proved against the Aeneas translation of `psp-flow` (`PspFlow/Funs.lean`), i.e. against
the same code the runtime executes. Nothing here is a hand-written model: the wait points,
their `event`/`resume` functions, `start`, `event`, `deliver` and
`AnyEvent.is_user_notification` are all generated.""",
    """Proved against the Aeneas translation of `psp-flow-dsl` (`PspFlowDsl/Funs.lean`), the
`flow!` macro's expansion. DERIVED from `PspProofs/Pay.lean` by `scripts/derive-dsl-proofs.py`;
do not edit. The three differences the macro output has: the user-notification predicate
is defined here rather than in the code, the decisions live in the `machine` module, and
`start` takes the entity handle.""")
s = s.replace(
    "attribute [local simp] machine.event machine.start machine.deliver AnyEvent.is_user_notification",
    """/-- The user is told something: P1 is stated against this. -/
def isUserNotification : AnyEvent → Bool
  | .ShowError _ => true
  | .Notify _ => true
  | _ => false

attribute [local simp] machine.event machine.start machine.deliver isUserNotification""")
s = re.sub(r"  values\.validate_pay_intent\n(  entity\.[^\n]*\n)+", "  machine.validate_pay_intent machine.build_req_pay\n", s)
s = s.replace("AnyEvent.is_user_notification e = ok true", "isUserNotification e = true")
s = s.replace("theorem P2_from_start {id payer s0 evs t}\n    (h0 : machine.start id payer = ok s0)",
              "theorem P2_from_start {txn s0 evs t}\n    (h0 : machine.start txn = ok s0)")
s = s.replace("theorem P6b_from_start {id payer s0 evs t a}\n    (h0 : machine.start id payer = ok s0)",
              "theorem P6b_from_start {txn s0 evs t a}\n    (h0 : machine.start txn = ok s0)")

os.makedirs(dst_dir, exist_ok=True)
open(os.path.join(dst_dir, "Pay.lean"), "w").write(s)
open(os.path.join(root, "psp-lean", "PspProofsDsl.lean"), "w").write("import PspProofsDsl.Pay\n")
print("wrote", os.path.join(dst_dir, "Pay.lean"))
