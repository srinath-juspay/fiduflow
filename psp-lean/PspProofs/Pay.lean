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

Conventions: Aeneas wraps every function in its `Result` monad (`ok`/`fail`/`div`); the
Rust `Result<Next, Mismatch>` of `deliver` appears inside it as `core.result.Result`.
-/

open Aeneas Aeneas.Std Result

namespace psp_flow
open machine events entity values
open driver_framework.machine

/-- One typed step: the wait point `s` resumed with a response of its event's type. -/
inductive Steps : PayerPay → Next PayerPay (Txn Terminal) → Prop
  | atGetPaymentIntent {w r n} : machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalGetPaymentIntentRawIntent.resume w r = ok n → Steps (.AtGetPaymentIntent w) n
  | atShowError {w r n} : machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalShowErrorTuple.resume w r = ok n → Steps (.AtShowError w) n
  | atCollectMpin {w r n} : machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalCollectMpinMpin.resume w r = ok n → Steps (.AtCollectMpin w) n
  | atReqPay {w r n} : machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalReqPayReqPayResponse.resume w r = ok n → Steps (.AtReqPay w) n
  | atNotifyRejected {w r n} : machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyRejected w) n
  | atAwaitCallback {w r n} : machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitCallbackCallback.resume w r = ok n → Steps (.AtAwaitCallback w) n
  | atNotifySuccess {w r n} : machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifySuccess w) n
  | atNotifyFailed {w r n} : machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyFailed w) n
  | atNotifyProcessing {w r n} : machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyProcessing w) n
  | atAwaitDeemed {w r n} : machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitDeemedResolutionResolution.resume w r = ok n → Steps (.AtAwaitDeemed w) n
  | atNotifyConfirmed {w r n} : machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyConfirmed w) n
  | atNotifyDeemedReversed {w r n} : machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyDeemedReversed w) n
  | atNotifyReversalStart {w r n} : machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyReversalStart w) n
  | atAwaitForcedReversal {w r n} : machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitForcedReversalTuple.resume w r = ok n → Steps (.AtAwaitForcedReversal w) n
  | atNotifyReversed {w r n} : machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume w r = ok n → Steps (.AtNotifyReversed w) n

/-- A complete run: the interactions performed from `s` until the terminal entity `t`. -/
inductive Reaches : PayerPay → List AnyEvent → Txn Terminal → Prop
  | done {s e t} :
      event s = ok e → Steps s (Next.Done t) → Reaches s [e] t
  | wait {s e s' evs t} :
      event s = ok e → Steps s (Next.Wait s') → Reaches s' evs t → Reaches s (e :: evs) t

/-! ## The boundary: `deliver` is sound with respect to the typed steps -/

/-- Inversion of the Aeneas `Result` bind: a successful `do` means the first action
    succeeded and the continuation succeeded on its value. -/
theorem bind_ok_inv {α β : Type} {m : Result α} {f : α → Result β} {y : β}
    (h : (do let x ← m; f x) = ok y) : ∃ x, m = ok x ∧ f x = ok y := by
  cases m <;> simp_all

set_option linter.unusedTactic false in
/-- Whatever `deliver` accepts is a typed step. (Proved before `resume` enters the simp
    set, so the step is matched as a call rather than unfolded.) The converse is by
    construction: `deliver` has exactly one arm per wait point, calling its `resume`. -/
theorem deliver_sound {s r n} (h : deliver s r = ok (core.result.Result.Ok n)) : Steps s n := by
  cases s <;> cases r <;> simp only [machine.deliver] at h <;> (first | (simp at h; done) | skip)
  all_goals (obtain ⟨x, hx, hfx⟩ := bind_ok_inv h; simp at hfx; subst hfx; constructor; exact hx)

-- The simp set that evaluates the generated code once the wait point is a constructor.
attribute [local simp] machine.event machine.start machine.deliver AnyEvent.is_user_notification
  machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalGetPaymentIntentRawIntent.resume machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalShowErrorTuple.resume machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalCollectMpinMpin.resume machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalReqPayReqPayResponse.resume
  machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitCallbackCallback.resume machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume
  machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitDeemedResolutionResolution.resume machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume
  machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitForcedReversalTuple.resume
  machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.resume
  machine.AtGetPaymentIntent.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalGetPaymentIntentRawIntent.event machine.AtShowError.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalShowErrorTuple.event machine.AtCollectMpin.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalCollectMpinMpin.event machine.AtReqPay.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalReqPayReqPayResponse.event
  machine.AtNotifyRejected.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event machine.AtAwaitCallback.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitCallbackCallback.event machine.AtNotifySuccess.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event machine.AtNotifyFailed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event
  machine.AtNotifyProcessing.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event machine.AtAwaitDeemed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitDeemedResolutionResolution.event machine.AtNotifyConfirmed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event
  machine.AtNotifyDeemedReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event machine.AtNotifyReversalStart.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event machine.AtAwaitForcedReversal.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalAwaitForcedReversalTuple.event
  machine.AtNotifyReversed.Insts.Driver_frameworkMachineWaitPointPayerPayTxnTerminalNotifyTuple.event
  values.validate_pay_intent
  entity.TxnInitiated.new entity.TxnInitiated.abandon entity.TxnInitiated.authenticate
  entity.TxnAuthenticated.build_req_pay entity.TxnAuthenticated.submit entity.TxnAuthenticated.reject
  entity.TxnSubmitted.succeed entity.TxnSubmitted.fail entity.TxnSubmitted.deem
  entity.TxnDeemed.confirm entity.TxnDeemed.reverse entity.TxnDeemed.escalate_reversal
  entity.TxnReversalPending.reversed
  entity.TxnSuccess.terminal entity.TxnFailed.terminal entity.TxnReversed.terminal entity.TxnAbandoned.terminal

/-- Case analysis through the generated `if`/`match` structure of a `resume`, then
    substitution and simplification. Every single-step lemma below is proved by this. -/
syntax "case_bash" : tactic
macro_rules
  | `(tactic| case_bash) => `(tactic|
      (all_goals (repeat' (split at *)))
      <;> (all_goals (try subst_vars))
      <;> (all_goals (try simp_all)))

/-! ## P1: the user is always told the outcome -/

theorem done_is_notification {s e t} (he : event s = ok e) (hs : Steps s (Next.Done t)) :
    AnyEvent.is_user_notification e = ok true := by
  cases hs <;> simp_all <;> case_bash

theorem P1_user_notified {s evs t} (h : Reaches s evs t) :
    ∃ e ∈ evs, AnyEvent.is_user_notification e = ok true := by
  induction h with
  | done he hs => exact ⟨_, List.mem_singleton.mpr rfl, done_is_notification he hs⟩
  | wait _ _ _ ih =>
    obtain ⟨e, hmem, hnot⟩ := ih
    exact ⟨e, List.mem_cons_of_mem _ hmem, hnot⟩

/-! ## P5: NPCI is never asked to pay without authentication -/

/-- `ReqPay` is only ever issued from `AtReqPay`, whose transaction is `Txn Authenticated`
    by the type of the wait point: the MPIN has been collected and the request built. -/
theorem P5_reqpay_only_when_authenticated {s ev} (h : event s = ok (AnyEvent.ReqPay ev)) :
    ∃ w : AtReqPay, s = PayerPay.AtReqPay w ∧ ev.req = w.req := by
  cases s <;> simp_all
  all_goals (subst_vars; rfl)

/-! ## P4: every transaction terminates: at most `measure s + 1` interactions remain -/

def measure : PayerPay → Nat
  | .AtGetPaymentIntent _ => 8
  | .AtCollectMpin _ => 7
  | .AtReqPay _ => 6
  | .AtAwaitCallback _ => 5
  | .AtNotifyProcessing _ => 4
  | .AtAwaitDeemed _ => 3
  | .AtNotifyReversalStart _ => 2
  | .AtAwaitForcedReversal _ => 1
  | _ => 0

theorem wait_decreases {s s'} (hs : Steps s (Next.Wait s')) : measure s' < measure s := by
  cases hs <;> simp_all [measure] <;> case_bash

theorem P4_terminates {s evs t} (h : Reaches s evs t) : evs.length ≤ measure s + 1 := by
  induction h with
  | done _ _ => simp
  | wait _ hs _ ih =>
    have := wait_decreases hs
    simp only [List.length_cons]
    omega

/-! ## P2: the MPIN is collected at most once per transaction -/

def isCollectMpin : AnyEvent → Bool
  | .CollectMpin _ => true
  | _ => false

def mpinBudget : PayerPay → Nat
  | .AtGetPaymentIntent _ => 1
  | .AtCollectMpin _ => 1
  | _ => 0

theorem mpin_budget_step {s e s'} (he : event s = ok e) (hs : Steps s (Next.Wait s')) :
    (if isCollectMpin e then 1 else 0) + mpinBudget s' ≤ mpinBudget s := by
  cases hs <;> simp_all [mpinBudget, isCollectMpin] <;> case_bash

theorem mpin_not_at_done {s e t} (he : event s = ok e) (hs : Steps s (Next.Done t)) :
    isCollectMpin e = false := by
  cases hs <;> simp_all [isCollectMpin] <;> case_bash

theorem P2_mpin_at_most_once {s evs t} (h : Reaches s evs t) :
    (evs.filter isCollectMpin).length ≤ mpinBudget s := by
  induction h with
  | done he hs =>
    have := mpin_not_at_done he hs
    simp [List.filter, this]
  | wait he hs _ ih =>
    have hb := mpin_budget_step he hs
    simp only [List.filter_cons]
    split <;> simp_all <;> omega

theorem P2_from_start {id payer s0 evs t}
    (h0 : machine.start id payer = ok s0) (h : Reaches s0 evs t) :
    (evs.filter isCollectMpin).length ≤ 1 := by
  have := P2_mpin_at_most_once h
  simp at h0
  subst h0
  simpa [mpinBudget] using this

/-! ## P6: entity value properties -/

def idOf : PayerPay → TxnId
  | .AtGetPaymentIntent w => w.txn.id
  | .AtShowError w => w.txn.id
  | .AtCollectMpin w => w.txn.id
  | .AtReqPay w => w.txn.id
  | .AtNotifyRejected w => w.txn.id
  | .AtAwaitCallback w => w.txn.id
  | .AtNotifySuccess w => w.txn.id
  | .AtNotifyFailed w => w.txn.id
  | .AtNotifyProcessing w => w.txn.id
  | .AtAwaitDeemed w => w.txn.id
  | .AtNotifyConfirmed w => w.txn.id
  | .AtNotifyDeemedReversed w => w.txn.id
  | .AtNotifyReversalStart w => w.txn.id
  | .AtAwaitForcedReversal w => w.txn.id
  | .AtNotifyReversed w => w.txn.id

/-- The amount is valid: non-zero and within the per-transaction cap. -/
def amountOk (a : Amount) : Prop := a ≠ 0#u64 ∧ ¬ (a > MAX_AMOUNT_PAISE)

/-- From the moment an intent has been validated, every request the entity carries has a
    valid amount. -/
def Inv : PayerPay → Prop
  | .AtGetPaymentIntent _ => True
  | .AtShowError _ => True
  | .AtCollectMpin w => amountOk w.intent.amount
  | .AtReqPay w => amountOk w.req.amount
  | .AtNotifyRejected w => amountOk w.req.amount
  | .AtAwaitCallback w => amountOk w.txn.data.req.amount
  | .AtNotifySuccess w => amountOk w.txn.data.req.amount
  | .AtNotifyFailed w => amountOk w.txn.data.req.amount
  | .AtNotifyProcessing w => amountOk w.txn.data.req.amount
  | .AtAwaitDeemed w => amountOk w.txn.data.req.amount
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

-- `idOf`/`Inv` are unfolded only after `s'` is known, otherwise `split` shreds the match.
theorem id_preserved_wait {s s'} (hs : Steps s (Next.Wait s')) : idOf s' = idOf s := by
  cases hs <;> simp_all <;> case_bash <;> (try subst_vars) <;> simp_all [idOf]

theorem id_preserved_done {s t} (hs : Steps s (Next.Done t)) : t.id = idOf s := by
  cases hs <;> simp_all <;> case_bash <;> simp_all [idOf]

/-- P6a: the terminal entity has the id the flow started with. -/
theorem P6a_id_preserved {s evs t} (h : Reaches s evs t) : t.id = idOf s := by
  induction h with
  | done _ hs => exact id_preserved_done hs
  | wait _ hs _ ih => rw [ih, id_preserved_wait hs]

theorem inv_wait {s s'} (hi : Inv s) (hs : Steps s (Next.Wait s')) : Inv s' := by
  cases hs <;> simp_all <;> case_bash <;> (try subst_vars) <;> simp_all [Inv, amountOk]

theorem inv_done {s t a} (hi : Inv s) (hs : Steps s (Next.Done t)) (ha : doneAmount t = some a) :
    amountOk a := by
  cases hs <;> simp_all <;> case_bash <;> simp_all [Inv, amountOk, doneAmount]

/-- P6b: every amount that reaches a terminal state passed validation. -/
theorem P6b_amount_validated {s evs t a} (hi : Inv s) (h : Reaches s evs t) (ha : doneAmount t = some a) :
    amountOk a := by
  induction h with
  | done _ hs => exact inv_done hi hs ha
  | wait _ hs _ ih => exact ih (inv_wait hi hs) ha

theorem P6b_from_start {id payer s0 evs t a}
    (h0 : machine.start id payer = ok s0) (h : Reaches s0 evs t) (ha : doneAmount t = some a) :
    a.val ≠ 0 ∧ a.val ≤ 10000000 := by
  simp at h0
  subst h0
  have := P6b_amount_validated (by simp [Inv]) h ha
  simp only [amountOk, MAX_AMOUNT_PAISE] at this
  obtain ⟨h1, h2⟩ := this
  constructor
  · intro hz; apply h1; scalar_tac
  · scalar_tac

end psp_flow
