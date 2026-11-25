import Aeneas
import PspFlow

/-!
# Properties of the UPI PSP pay flow

Proved against the Aeneas translation of `psp-flow` (`PspFlow/Funs.lean`), i.e. against
the same code the runtime executes. Nothing here is a hand-written model: the wait points,
their `event`/`resume` functions, `start`, `event`, `deliver` and
`AnyEvent.is_user_notification` are all generated.

Each wait point's `resume` takes exactly the response type of its event, so "every response
is handled" (the old P3) is now a typing fact; what remains to prove about the boundary is
that `deliver` only ever calls `resume` (`deliver_sound`).

## Parameterized by FlowConfig

All properties are proved for ALL possible `FlowConfig` values via universal quantification.
The config parameter affects branching in `AtCollectMpin` (HighValueMode) and `AtAwaitDeemed`
(DeemedPolicy), but the proofs hold regardless of config.

Conventions: Aeneas wraps every function in its `Result` monad (`ok`/`fail`/`div`); the
Rust `Result<Next, Mismatch>` of `deliver` appears inside it as `core.result.Result`.
-/

open Aeneas Aeneas.Std Result

set_option maxHeartbeats 800000
set_option linter.unusedTactic false

namespace psp_flow
open machine events entity values
open driver_framework.machine

/-- One typed step: the wait point `s` resumed with a response of its event's type, using config `cfg`. -/
inductive Steps (cfg : FlowConfig) : PayerPay → Next PayerPay (Txn Terminal) → Prop
  | atInit {w r n} : machine.AtInit.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigInitPayerId.resume w r cfg = ok n → Steps cfg (.AtInit w) n
  | atGetPaymentIntent {w r n} : machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigGetPaymentIntentRawIntent.resume w r cfg = ok n → Steps cfg (.AtGetPaymentIntent w) n
  | atShowError {w r n} : machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigShowErrorTuple.resume w r cfg = ok n → Steps cfg (.AtShowError w) n
  | atCollectMpin {w r n} : machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigCollectMpinMpin.resume w r cfg = ok n → Steps cfg (.AtCollectMpin w) n
  | atAwaitOtp {w r n} : machine.AtAwaitOtp.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitOtpOtpResult.resume w r cfg = ok n → Steps cfg (.AtAwaitOtp w) n
  | atAwaitApproval {w r n} : machine.AtAwaitApproval.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitApprovalApprovalResult.resume w r cfg = ok n → Steps cfg (.AtAwaitApproval w) n
  | atReqPay {w r n} : machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigReqPayReqPayResponse.resume w r cfg = ok n → Steps cfg (.AtReqPay w) n
  | atNotifyRejected {w r n} : machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyRejected w) n
  | atAwaitCallback {w r n} : machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitCallbackCallback.resume w r cfg = ok n → Steps cfg (.AtAwaitCallback w) n
  | atNotifySuccess {w r n} : machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifySuccess w) n
  | atNotifyFailed {w r n} : machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyFailed w) n
  | atNotifyProcessing {w r n} : machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyProcessing w) n
  | atAwaitDeemed {w r n} : machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitDeemedResolutionResolution.resume w r cfg = ok n → Steps cfg (.AtAwaitDeemed w) n
  | atEscalateToOps {w r n} : machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume w r cfg = ok n → Steps cfg (.AtEscalateToOps w) n
  | atNotifyConfirmed {w r n} : machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyConfirmed w) n
  | atNotifyDeemedReversed {w r n} : machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyDeemedReversed w) n
  | atNotifyReversalStart {w r n} : machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyReversalStart w) n
  | atAwaitForcedReversal {w r n} : machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitForcedReversalTuple.resume w r cfg = ok n → Steps cfg (.AtAwaitForcedReversal w) n
  | atNotifyReversed {w r n} : machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume w r cfg = ok n → Steps cfg (.AtNotifyReversed w) n

/-- A complete run: the interactions performed from `s` until the terminal entity `t`, using config `cfg`. -/
inductive Reaches (cfg : FlowConfig) : PayerPay → List AnyEvent → Txn Terminal → Prop
  | done {s e t} :
      event s = ok e → Steps cfg s (Next.Done t) → Reaches cfg s [e] t
  | wait {s e s' evs t} :
      event s = ok e → Steps cfg s (Next.Wait s') → Reaches cfg s' evs t → Reaches cfg s (e :: evs) t

/-! ## The boundary: `deliver` is sound with respect to the typed steps -/

theorem bind_ok_inv {α β : Type} {m : Result α} {f : α → Result β} {y : β}
    (h : (do let x ← m; f x) = ok y) : ∃ x, m = ok x ∧ f x = ok y := by
  cases m <;> simp_all

theorem deliver_sound {cfg s r n} (h : deliver s r cfg = ok (core.result.Result.Ok n)) : Steps cfg s n := by
  cases s <;> cases r <;> simp only [machine.deliver] at h <;> (first | (simp at h; done) | skip)
  all_goals (obtain ⟨x, hx, hfx⟩ := bind_ok_inv h; simp at hfx; subst hfx; constructor; exact hx)

-- The simp set that evaluates the generated code once the wait point is a constructor.
attribute [local simp] machine.event machine.start machine.deliver AnyEvent.is_user_notification
  machine.AtInit.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigInitPayerId.resume machine.AtInit.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigInitPayerId.event
  machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigGetPaymentIntentRawIntent.resume machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigShowErrorTuple.resume machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigCollectMpinMpin.resume
  machine.AtAwaitOtp.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitOtpOtpResult.resume
  machine.AtAwaitApproval.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitApprovalApprovalResult.resume
  machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigReqPayReqPayResponse.resume
  machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitCallbackCallback.resume machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume
  machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitDeemedResolutionResolution.resume
  machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume
  machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume
  machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitForcedReversalTuple.resume
  machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.resume
  machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigGetPaymentIntentRawIntent.event machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigShowErrorTuple.event machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigCollectMpinMpin.event
  machine.AtAwaitOtp.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitOtpOtpResult.event
  machine.AtAwaitApproval.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitApprovalApprovalResult.event
  machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigReqPayReqPayResponse.event
  machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitCallbackCallback.event machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event
  machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitDeemedResolutionResolution.event
  machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.event
  machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event
  machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitForcedReversalTuple.event
  machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigNotifyTuple.event
  values.validate_pay_intent
  entity.TxnInitiated.new entity.TxnInitiated.abandon entity.TxnInitiated.authenticate
  entity.TxnAuthenticated.build_req_pay entity.TxnAuthenticated.submit entity.TxnAuthenticated.reject
  entity.TxnSubmitted.succeed entity.TxnSubmitted.fail entity.TxnSubmitted.deem
  entity.TxnDeemed.confirm entity.TxnDeemed.reverse entity.TxnDeemed.escalate_reversal
  entity.TxnReversalPending.reversed
  entity.TxnSuccess.terminal entity.TxnFailed.terminal entity.TxnReversed.terminal entity.TxnAbandoned.terminal
  machine.MAX_OPS_RETRIES

/-- When retries > 0, subtraction succeeds -/
theorem u8_sub_one_ok {x : U8} (h : 0 < x.val) :
    ∃ z : U8, x - 1#u8 = ok z ∧ z.val = x.val - 1 := by
  have hbounds : x.val < 256 := x.hBounds
  have hlt : x.val - 1 < 256 := Nat.sub_lt_right_of_lt_add (by omega) (by omega)
  use ⟨BitVec.ofNat 8 (x.val - 1)⟩
  constructor
  · simp only [HSub.hSub, Sub.sub, UScalar.sub]
    split_ifs with hcmp
    · simp at hcmp; omega
    · rfl
  · simp only [UScalar.val]
    exact Nat.mod_eq_of_lt hlt

/-- A bind returning Wait can't equal Done -/
theorem bind_wait_ne_done {α : Type} {m : Result α} {f : α → PayerPay} {t : Txn Terminal} :
    (do let i ← m; ok (Next.Wait (f i))) ≠ ok (Next.Done t) := by
  intro h
  cases m using Result.cases with
  | ret x => simp only [bind_ok, Result.ok.injEq] at h; cases h
  | vis e k => simp only [bind_vis] at h; exact ok_not_vis h.symm
  | div => simp only [bind_div] at h; exact ok_not_div h.symm

/-- AtEscalateToOps.resume never produces Next.Done -/
theorem atEscalateToOps_no_done {w : AtEscalateToOps} {r : OpsDecision} {cfg : FlowConfig} {t : Txn Terminal} :
    machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume w r cfg ≠ ok (Next.Done t) := by
  intro h
  simp only [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume] at h
  cases r with
  | Reverse =>
    simp only at h
    have ⟨t', ht', hn⟩ := bind_ok_inv h
    simp only [Result.ok.injEq] at hn
    cases hn
  | Retry =>
    simp only at h
    split at h
    · exact bind_wait_ne_done h
    · have ⟨t', ht', hn⟩ := bind_ok_inv h
      simp only [Result.ok.injEq] at hn
      cases hn
  | Fail =>
    simp only [Result.ok.injEq] at h
    cases h

/-- Case analysis tactic -/
syntax "case_bash" : tactic
macro_rules
  | `(tactic| case_bash) => `(tactic|
      (all_goals (repeat' (split at *)))
      <;> (all_goals (try subst_vars))
      <;> (all_goals (try simp_all))
      <;> (all_goals (try (subst_vars; assumption)))
      <;> (all_goals (try scalar_tac))
      <;> (all_goals (try omega))
      <;> (all_goals (try (subst_vars; simp_all))))

/-! ## P1: the user is always told the outcome -/

theorem done_is_notification {cfg s e t} (he : event s = ok e) (hs : Steps cfg s (Next.Done t)) :
    AnyEvent.is_user_notification e = ok true := by
  cases hs with
  | atEscalateToOps hres => exfalso; exact atEscalateToOps_no_done hres
  | _ => simp_all <;> case_bash

theorem P1_user_notified {cfg s evs t} (h : Reaches cfg s evs t) :
    ∃ e ∈ evs, AnyEvent.is_user_notification e = ok true := by
  induction h with
  | done he hs => exact ⟨_, List.mem_singleton.mpr rfl, done_is_notification he hs⟩
  | wait _ _ _ ih => obtain ⟨e, hmem, hnot⟩ := ih; exact ⟨e, List.mem_cons_of_mem _ hmem, hnot⟩

/-! ## P5: NPCI is never asked to pay without authentication -/

theorem P5_reqpay_only_when_authenticated {s ev} (h : event s = ok (AnyEvent.ReqPay ev)) :
    ∃ w : AtReqPay, s = PayerPay.AtReqPay w ∧ ev.req = w.req := by
  cases s <;> simp_all; all_goals (subst_vars; rfl)

/-! ## P4: every transaction terminates -/

def measure : PayerPay → Nat
  | .AtInit _ => 14
  | .AtGetPaymentIntent _ => 13
  | .AtCollectMpin _ => 12
  | .AtAwaitOtp _ => 11
  | .AtAwaitApproval _ => 11
  | .AtReqPay _ => 10
  | .AtAwaitCallback _ => 9
  | .AtNotifyProcessing _ => 8
  | .AtAwaitDeemed _ => 7
  | .AtEscalateToOps w => 3 + w.retries_remaining.val
  | .AtNotifyReversalStart _ => 2
  | .AtAwaitForcedReversal _ => 1
  | _ => 0

theorem wait_decreases {cfg s s'} (hs : Steps cfg s (Next.Wait s')) : measure s' < measure s := by
  cases hs with
  | atEscalateToOps hres =>
    rename_i w r
    simp only [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume] at hres
    cases r with
    | Reverse =>
      simp only at hres
      have ⟨t, ht, hn⟩ := bind_ok_inv hres
      simp only [Result.ok.injEq, Next.Wait.injEq] at hn
      subst hn
      simp only [measure]
      have hnn : (0 : Nat) ≤ w.retries_remaining.val := Nat.zero_le _
      omega
    | Retry =>
      simp only at hres
      split at hres
      · rename_i hpos
        have ⟨z, hz, hval⟩ := u8_sub_one_ok hpos
        simp only [hz, bind_ok, Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres
        simp only [measure]
        have hpos' : 0 < w.retries_remaining.val := hpos
        calc 3 + z.val = 3 + (w.retries_remaining.val - 1) := by rw [hval]
          _ < 3 + w.retries_remaining.val := by omega
      · have ⟨t, ht, hn⟩ := bind_ok_inv hres
        simp only [Result.ok.injEq, Next.Wait.injEq] at hn
        subst hn
        simp only [measure]
        have hnn : (0 : Nat) ≤ w.retries_remaining.val := Nat.zero_le _
        omega
    | Fail =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres
      simp only [measure]
      have hnn : (0 : Nat) ≤ w.retries_remaining.val := Nat.zero_le _
      omega
  | atAwaitDeemed hres =>
    rename_i w res
    simp only [machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigAwaitDeemedResolutionResolution.resume] at hres
    cases res with
    | DeemedSuccess _ =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres; simp [measure]
    | DeemedFailed =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres; simp [measure]
    | DeadlineExpired =>
      simp only at hres
      split at hres
      · -- WaitForResolution: has bind expression
        have ⟨t, ht, hn⟩ := bind_ok_inv hres
        simp only [Result.ok.injEq, Next.Wait.injEq] at hn
        subst hn; simp [measure]
      · -- AutoReverse
        simp only [Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres; simp [measure]
      · -- Escalate
        simp only [Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres; simp [measure, machine.MAX_OPS_RETRIES]
  | _ =>
    simp_all [measure]
    all_goals (first | case_bash | decide | omega)

theorem P4_terminates {cfg s evs t} (h : Reaches cfg s evs t) : evs.length ≤ measure s + 1 := by
  induction h with
  | done _ _ => simp
  | wait _ hs _ ih => have := wait_decreases hs; simp only [List.length_cons]; omega

/-! ## P2: the MPIN is collected at most once -/

def isCollectMpin : AnyEvent → Bool
  | .CollectMpin _ => true
  | _ => false

def mpinBudget : PayerPay → Nat
  | .AtInit _ => 1
  | .AtGetPaymentIntent _ => 1
  | .AtCollectMpin _ => 1
  | _ => 0

theorem mpin_budget_step {cfg s e s'} (he : event s = ok e) (hs : Steps cfg s (Next.Wait s')) :
    (if isCollectMpin e then 1 else 0) + mpinBudget s' ≤ mpinBudget s := by
  cases hs with
  | atEscalateToOps hres =>
    rename_i w r
    simp only [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume] at hres
    simp [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.event] at he
    subst he
    simp only [isCollectMpin]
    -- All outcomes of atEscalateToOps have mpinBudget = 0
    cases r with
    | Reverse =>
      have ⟨t, _, hn⟩ := bind_ok_inv hres
      simp only [Result.ok.injEq, Next.Wait.injEq] at hn
      subst hn; simp [mpinBudget]
    | Retry =>
      simp only at hres
      split at hres
      · rename_i hpos
        have ⟨z, hz, _⟩ := u8_sub_one_ok hpos
        simp only [hz, bind_ok, Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres; simp [mpinBudget]
      · have ⟨t, _, hn⟩ := bind_ok_inv hres
        simp only [Result.ok.injEq, Next.Wait.injEq] at hn
        subst hn; simp [mpinBudget]
    | Fail =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres; simp [mpinBudget]
  | _ => simp_all [mpinBudget, isCollectMpin] <;> case_bash

theorem mpin_not_at_done {cfg s e t} (he : event s = ok e) (hs : Steps cfg s (Next.Done t)) :
    isCollectMpin e = false := by
  cases hs with
  | atEscalateToOps hres => exfalso; exact atEscalateToOps_no_done hres
  | _ => simp_all [isCollectMpin] <;> case_bash

theorem P2_mpin_at_most_once {cfg s evs t} (h : Reaches cfg s evs t) :
    (evs.filter isCollectMpin).length ≤ mpinBudget s := by
  induction h with
  | done he hs => have := mpin_not_at_done he hs; simp [List.filter, this]
  | wait he hs _ ih => have hb := mpin_budget_step he hs; simp only [List.filter_cons]; split <;> simp_all <;> omega

theorem P2_from_start {cfg id payer s0 evs t}
    (h0 : machine.start id payer = ok s0) (h : Reaches cfg s0 evs t) :
    (evs.filter isCollectMpin).length ≤ 1 := by
  have := P2_mpin_at_most_once h; simp at h0; subst h0; simpa [mpinBudget] using this

/-! ## P6: entity value properties -/

def idOf : PayerPay → TxnId
  | .AtInit w => w.id
  | .AtGetPaymentIntent w => w.txn.id
  | .AtShowError w => w.txn.id
  | .AtCollectMpin w => w.txn.id
  | .AtAwaitOtp w => w.txn.id
  | .AtAwaitApproval w => w.txn.id
  | .AtReqPay w => w.txn.id
  | .AtNotifyRejected w => w.txn.id
  | .AtAwaitCallback w => w.txn.id
  | .AtNotifySuccess w => w.txn.id
  | .AtNotifyFailed w => w.txn.id
  | .AtNotifyProcessing w => w.txn.id
  | .AtAwaitDeemed w => w.txn.id
  | .AtEscalateToOps w => w.txn.id
  | .AtNotifyConfirmed w => w.txn.id
  | .AtNotifyDeemedReversed w => w.txn.id
  | .AtNotifyReversalStart w => w.txn.id
  | .AtAwaitForcedReversal w => w.txn.id
  | .AtNotifyReversed w => w.txn.id

def amountOk (a : Amount) : Prop := a ≠ 0#u64 ∧ ¬ (a > MAX_AMOUNT_PAISE)

def Inv : PayerPay → Prop
  | .AtInit _ => True
  | .AtGetPaymentIntent _ => True
  | .AtShowError _ => True
  | .AtCollectMpin w => amountOk w.intent.amount
  | .AtAwaitOtp w => amountOk w.req.amount
  | .AtAwaitApproval w => amountOk w.req.amount
  | .AtReqPay w => amountOk w.req.amount
  | .AtNotifyRejected w => amountOk w.req.amount
  | .AtAwaitCallback w => amountOk w.txn.data.req.amount
  | .AtNotifySuccess w => amountOk w.txn.data.req.amount
  | .AtNotifyFailed w => amountOk w.txn.data.req.amount
  | .AtNotifyProcessing w => amountOk w.txn.data.req.amount
  | .AtAwaitDeemed w => amountOk w.txn.data.req.amount
  | .AtEscalateToOps w => amountOk w.txn.data.req.amount
  | .AtNotifyConfirmed w => amountOk w.txn.data.req.amount
  | .AtNotifyDeemedReversed w => amountOk w.txn.data.req.amount
  | .AtNotifyReversalStart w => amountOk w.txn.data.req.amount
  | .AtAwaitForcedReversal w => amountOk w.txn.data.req.amount
  | .AtNotifyReversed w => amountOk w.txn.data.req.amount

def doneAmount : Txn Terminal → Option Amount
  | ⟨_, .Success s⟩ => some s.req.amount
  | ⟨_, .Failed f⟩ => some f.req.amount
  | ⟨_, .Reversed r⟩ => some r.req.amount
  | ⟨_, .Abandoned _⟩ => none

theorem id_preserved_wait {cfg s s'} (hs : Steps cfg s (Next.Wait s')) : idOf s' = idOf s := by
  cases hs with
  | atEscalateToOps hres =>
    rename_i w r
    simp only [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume] at hres
    cases r with
    | Reverse =>
      have ⟨t, ht, hn⟩ := bind_ok_inv hres
      simp only [Result.ok.injEq, Next.Wait.injEq] at hn
      subst hn
      simp [idOf, entity.TxnDeemed.escalate_reversal] at ht ⊢
      subst ht; rfl
    | Retry =>
      simp only at hres
      split at hres
      · rename_i hpos
        have ⟨z, hz, _⟩ := u8_sub_one_ok hpos
        simp only [hz, bind_ok, Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres; simp [idOf]
      · have ⟨t, ht, hn⟩ := bind_ok_inv hres
        simp only [Result.ok.injEq, Next.Wait.injEq] at hn
        subst hn
        simp [idOf, entity.TxnDeemed.escalate_reversal] at ht ⊢
        subst ht; rfl
    | Fail =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres; simp [idOf]
  | _ =>
    simp_all [idOf]
    all_goals (first | rfl | (subst_vars; simp [idOf]) | case_bash)

theorem id_preserved_done {cfg s t} (hs : Steps cfg s (Next.Done t)) : t.id = idOf s := by
  cases hs with
  | atEscalateToOps hres => exfalso; exact atEscalateToOps_no_done hres
  | _ => simp_all [idOf] <;> case_bash

theorem P6a_id_preserved {cfg s evs t} (h : Reaches cfg s evs t) : t.id = idOf s := by
  induction h with
  | done _ hs => exact id_preserved_done hs
  | wait _ hs _ ih => rw [ih, id_preserved_wait hs]

theorem inv_wait {cfg s s'} (hi : Inv s) (hs : Steps cfg s (Next.Wait s')) : Inv s' := by
  cases hs with
  | atEscalateToOps hres =>
    rename_i w r
    simp only [machine.AtEscalateToOps.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalFlowConfigEscalateToOpsOpsDecision.resume] at hres
    simp only [Inv, amountOk] at hi ⊢
    -- The result preserves the txn, so the amount is preserved
    cases r with
    | Reverse =>
      have ⟨t, ht, hn⟩ := bind_ok_inv hres
      simp only [Result.ok.injEq, Next.Wait.injEq] at hn
      subst hn
      simp [entity.TxnDeemed.escalate_reversal] at ht
      subst ht; exact hi
    | Retry =>
      simp only at hres
      split at hres
      · rename_i hpos
        have ⟨z, hz, _⟩ := u8_sub_one_ok hpos
        simp only [hz, bind_ok, Result.ok.injEq, Next.Wait.injEq] at hres
        subst hres; exact hi
      · have ⟨t, ht, hn⟩ := bind_ok_inv hres
        simp only [Result.ok.injEq, Next.Wait.injEq] at hn
        subst hn
        simp [entity.TxnDeemed.escalate_reversal] at ht
        subst ht; exact hi
    | Fail =>
      simp only [Result.ok.injEq, Next.Wait.injEq] at hres
      subst hres; exact hi
  | _ => simp_all [Inv, amountOk] <;> case_bash

theorem inv_done {cfg s t a} (hi : Inv s) (hs : Steps cfg s (Next.Done t)) (ha : doneAmount t = some a) :
    amountOk a := by
  cases hs with
  | atEscalateToOps hres => exfalso; exact atEscalateToOps_no_done hres
  | _ =>
    simp_all [Inv, amountOk, doneAmount]
    all_goals (try (subst_vars; simp_all; done))
    all_goals case_bash

theorem P6b_amount_validated {cfg s evs t a} (hi : Inv s) (h : Reaches cfg s evs t) (ha : doneAmount t = some a) :
    amountOk a := by
  induction h with
  | done _ hs => exact inv_done hi hs ha
  | wait _ hs _ ih => exact ih (inv_wait hi hs) ha

theorem P6b_from_start {cfg id payer s0 evs t a}
    (h0 : machine.start id payer = ok s0) (h : Reaches cfg s0 evs t) (ha : doneAmount t = some a) :
    a.val ≠ 0 ∧ a.val ≤ 10000000 := by
  simp at h0; subst h0
  have := P6b_amount_validated (by simp [Inv]) h ha
  simp only [amountOk, MAX_AMOUNT_PAISE] at this
  obtain ⟨h1, h2⟩ := this
  constructor
  · intro hz; apply h1; scalar_tac
  · scalar_tac

end psp_flow
