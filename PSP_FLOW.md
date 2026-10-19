# UPI PSP pay flow: requirements

This is the specification that `psp-flow`, `psp-app` and `psp-lean` implement. It describes
the "Pay (push)" flow of a UPI Payment Service Provider from the PSP's own point of view,
the properties the flow must have, and what the runtime must guarantee about persistence.
The last section maps every requirement to the code, test or theorem that discharges it.

The flow is written in the interaction-tree style: a process is a tree whose nodes are
interactions with the outside world and whose leaves are terminal states. That style is the
authoring form the DSL will offer; what is in `psp-flow` today is its machine form, which
the DSL's `#[flow]` macro is meant to generate.

---

## 1. Scope and parties

The PSP is the application layer, the thing a user opens to pay. From the PSP's perspective
there are exactly two external parties, both opaque:

| Party | What the PSP does with it |
|---|---|
| **User** | asks for a payment intent, asks for the MPIN, shows errors and status notices |
| **NPCI** | sends the pay request, waits for the synchronous ack, waits for the asynchronous callback, waits for deemed resolution and forced reversal |

Banks are behind NPCI and never appear. The PSP only ever sees user actions and NPCI
request/response pairs.

Persistence is not the flow author's concern. The runtime must save the flow's state at
every interaction boundary and restore it when the answer arrives (section 7).

## 2. Notation

```
Vis(event, λ(response) → ...)   -- an interaction: ask, then continue with the answer
Tau(expr)                       -- pure internal computation
Ret(value)                      -- terminal state
f >>= λ(x) → ...                -- sequential composition of subflows
```

Every `Vis` with a response that can be delayed carries a timeout or deadline, and the
expiry is one of the response's variants, so handling it is not optional.

## 3. The entity: a UPI transaction

A transaction has exactly one state at any time. The state must be queryable outside the
flow (support, reconciliation, later flows such as dispute or refund), and the data a state
carries is only the data that exists in that state.

| State | Data | Terminal |
|---|---|---|
| `Initiated` | payer | |
| `Authenticated` | payer, validated intent (payee, amount), MPIN | |
| `Submitted` | the pay request sent to NPCI | |
| `Deemed` | the pay request | |
| `ReversalPending` | the pay request | |
| `Success` | the pay request, optional debit reference, credit reference | yes |
| `Failed` | the pay request, reason | yes |
| `Reversed` | the pay request | yes |
| `Abandoned` | payer | yes |

Transitions happen only inside the flow; nothing else may change a transaction's state.

Values are refined: amounts are in paise, non-zero and at most Rs 1,00,000; reasons and
statuses are closed enumerations; nothing is a free-form string.

## 4. Interactions

Each interaction is defined together with the type of its response.

| Party | Event | Payload | Response |
|---|---|---|---|
| User | `GetPaymentIntent` | payer | raw intent: payee, amount (unvalidated) |
| User | `ShowError` | reason | acknowledgement |
| User | `CollectMpin` | payer, validated intent | MPIN |
| User | `Notify` | status | acknowledgement |
| NPCI | `ReqPay` | pay request | `Ack` or `Rejected { reason }` |
| NPCI | `AwaitCallback` | timeout 30 s | `Success { debit_ref, credit_ref }`, `Failed { reason }`, `Deemed`, `Timeout` |
| NPCI | `AwaitDeemedResolution` | deadline T+3 days | `DeemedSuccess { credit_ref }`, `DeemedFailed`, `DeadlineExpired` |
| NPCI | `AwaitForcedReversal` | deadline T+5 days | acknowledgement |

Statuses shown to the user: successful, failed (with reason), processing, confirmed,
reversed, reversal in progress.

Pure decisions used by the flow:

- `validate_pay_intent(payer, raw)`: rejects a zero or over-cap amount (`InvalidAmount`),
  an unknown payee (`UnknownPayee`), and paying oneself (`PayeeIsPayer`); otherwise yields a
  validated intent.
- `build_req_pay`: builds the NPCI request from an *authenticated* transaction. It must not
  be possible to build a request from anything less.

## 5. Subflows

### `collect_intent`: get and validate the payment intent

```
collect_intent(payer) =
  Vis(User.GetPaymentIntent { payer },  λ(intent) →
    Tau(validate_pay_intent(intent) →
      match {
        Invalid { reason } → Vis(User.ShowError { reason },  λ(_) → Ret(TxnState::Abandoned)),
        Valid { payee_vpa, amount } → Ret(ValidIntent { payee_vpa, amount })
      }))
```

### `authenticate_payer`: collect the MPIN

```
authenticate_payer() = Vis(User.CollectMPIN,  λ(mpin) → Ret(mpin))
```

### `submit_to_npci`: send the request and handle the synchronous answer

```
submit_to_npci(req) =
  Vis(NPCI.ReqPay { req },  λ(npci_resp) →
    Tau(match npci_resp {
      NPCI.Ack → npci_callback(),
      NPCI.Rejected { reason } →
        Vis(User.Notify { status: "Payment failed", reason },  λ(_) → Ret(TxnState::Failed { reason }))
    }))
```

### `npci_callback`: await and handle the asynchronous answer

```
npci_callback() =
  Vis(NPCI.AwaitCallback { timeout: 30s },  λ(callback) →
    Tau(match callback {
      NPCI.Success { debit_ref, credit_ref } →
        Vis(User.Notify { status: "Payment successful" },  λ(_) → Ret(TxnState::Success { debit_ref, credit_ref })),
      NPCI.Failed { reason } →
        Vis(User.Notify { status: "Payment failed", reason },  λ(_) → Ret(TxnState::Failed { reason })),
      NPCI.Deemed →
        Vis(User.Notify { status: "Processing, may take up to 3 days" },  λ(_) → deemed_resolution()),
      Timeout →
        Vis(User.Notify { status: "Payment processing" },  λ(_) → deemed_resolution())
    }))
```

### `deemed_resolution`: handle the DEEMED state until terminal

Reused by both the `Deemed` and `Timeout` branches.

```
deemed_resolution() =
  Vis(NPCI.AwaitDeemedResolution { deadline: T+3 },  λ(resolution) →
    Tau(match resolution {
      NPCI.DeemedSuccess { credit_ref } →
        Vis(User.Notify { status: "Payment confirmed" },  λ(_) → Ret(TxnState::Success { credit_ref })),
      NPCI.DeemedFailed →
        Vis(User.Notify { status: "Payment reversed" },  λ(_) → Ret(TxnState::Reversed)),
      NPCI.DeemedDeadlineExpired → forced_reversal()
    }))
```

### `forced_reversal`: RBI-mandated reversal after the deemed deadline

```
forced_reversal() =
  Vis(User.Notify { status: "Reversal in progress" },  λ(_) →
    Vis(NPCI.AwaitForcedReversal { deadline: T+5 },  λ(_) →
      Vis(User.Notify { status: "Payment reversed" },  λ(_) → Ret(TxnState::Reversed))))
```

## 6. The main flow

```
PayerPSP_Pay(payer_id, payee_vpa, amount) =
  collect_intent(payee_vpa, amount) >>= λ(intent_result) →
    Tau(match intent_result {
      TxnState::Abandoned → Ret(TxnState::Abandoned),
      ValidIntent { payee_vpa, amount } →
        authenticate_payer() >>= λ(mpin) →
          Tau(build_req_pay(payer_id, payee_vpa, amount, mpin) → req → submit_to_npci(req))
    })
```

Read top-down: collect intent, validate, authenticate, build the request, submit, handle
the callback, handle deemed if needed, handle forced reversal if needed.

### Composition

```
PayerPSP_Pay
  ├── collect_intent          -- user interaction + validation
  ├── authenticate_payer      -- MPIN collection
  └── submit_to_npci          -- NPCI request + sync response
        └── npci_callback     -- async response handling
              ├── (success)   -- terminal
              ├── (failed)    -- terminal
              └── deemed_resolution
                    ├── (success)    -- terminal
                    ├── (failed)     -- terminal
                    └── forced_reversal
                          └── (reversed) -- terminal
```

`deemed_resolution` is reused by two branches; `forced_reversal`, `npci_callback`,
`collect_intent` and `authenticate_payer` are meant to be reusable by other flows (collect,
mandate, refund, dispute).

### The whole flow inlined, as one tree

```
PayerPSP_Pay(payer_id) =
  Vis(User.GetPaymentIntent { payer_id },  λ(intent) →
    Tau(validate_pay_intent(intent) → match {
      Invalid { reason } →
        Vis(User.ShowError { reason },  λ(_) → Ret(TxnState::Abandoned)),
      Valid { payee_vpa, amount } →
        Vis(User.CollectMPIN { payer_id, payee_vpa, amount },  λ(mpin) →
          Vis(NPCI.ReqPay { payer_id, payee_vpa, amount, mpin },  λ(npci_resp) →
            Tau(match npci_resp {
              NPCI.Rejected { reason } →
                Vis(User.Notify { "Payment failed", reason },  λ(_) → Ret(TxnState::Failed { reason })),
              NPCI.Ack →
                Vis(NPCI.AwaitCallback { timeout: 30s },  λ(callback) →
                  Tau(match callback {
                    NPCI.Success { debit_ref, credit_ref } →
                      Vis(User.Notify { "Payment successful" },  λ(_) → Ret(TxnState::Success { debit_ref, credit_ref })),
                    NPCI.Failed { reason } →
                      Vis(User.Notify { "Payment failed", reason },  λ(_) → Ret(TxnState::Failed { reason })),
                    NPCI.Deemed | Timeout →
                      Vis(User.Notify { "Processing" },  λ(_) →
                        Vis(NPCI.AwaitDeemedResolution { deadline: T+3 },  λ(resolution) →
                          Tau(match resolution {
                            NPCI.DeemedSuccess { credit_ref } →
                              Vis(User.Notify { "Payment confirmed" },  λ(_) → Ret(TxnState::Success { credit_ref })),
                            NPCI.DeemedFailed →
                              Vis(User.Notify { "Payment reversed" },  λ(_) → Ret(TxnState::Reversed)),
                            NPCI.DeemedDeadlineExpired →
                              Vis(User.Notify { "Reversal in progress" },  λ(_) →
                                Vis(NPCI.AwaitForcedReversal { deadline: T+5 },  λ(_) →
                                  Vis(User.Notify { "Payment reversed" },  λ(_) → Ret(TxnState::Reversed))))
                          })))
                  }))
            })))
    }))
```

Fifteen `Vis` nodes, seven `Ret` leaves.

### The seven paths

| # | Path | Terminal |
|---|---|---|
| 1 | intent invalid → show error | `Abandoned` |
| 2 | NPCI rejects synchronously → notify | `Failed` |
| 3 | ack → callback success → notify | `Success` |
| 4 | ack → callback failed → notify | `Failed` |
| 5 | ack → deemed or timeout → notify → deemed success → notify | `Success` |
| 6 | ack → deemed or timeout → notify → deemed failed → notify | `Reversed` |
| 7 | ack → deemed or timeout → notify → deadline expired → notify → forced reversal → notify | `Reversed` |

## 7. Properties the flow must have

Each of these is a requirement, not an observation. They must hold for every path, and
they must be proved about the code that runs, not about a separate model.

| Id | Property | What it prevents |
|---|---|---|
| P1 | The user is always told the outcome: every path to a terminal state passes through `User.Notify` or `User.ShowError`. | silent failures, double payments |
| P2 | The MPIN is collected at most once per transaction, and never on the abandoned path. | unauthorised debit, replay |
| P3 | Every response an interaction can produce is handled. | dropped transactions |
| P4 | Every transaction reaches a terminal state in a bounded number of interactions: no cycles, every wait has a deadline. | stuck money |
| P5 | NPCI is never asked to pay without authentication: `NPCI.ReqPay` is only reachable after `User.CollectMPIN`. | unauthorised payment |
| P6a | The transaction id is preserved from start to terminal state. | records attached to the wrong transaction |
| P6b | Any amount that reaches a terminal state passed validation: non-zero and within the cap. | NPCI or the ledger ever seeing an invalid amount |

Properties must compose with subflows: a property proved for `forced_reversal` once is
inherited by every flow that reuses it.

## 8. Persistence and runtime requirements

- **Save before every wait.** Before performing any interaction, the runtime stores the
  flow's bookmark, which carries the transaction, to the database.
- **Load on every response.** When a response arrives, the runtime loads the bookmark from
  the database, not from memory, advances the flow, and stores the new bookmark, all in one
  transaction.
- **Crash recovery.** After a restart the runtime can list every waiting instance and the
  interaction it is waiting on, from the database alone.
- **Rollback.** A response that does not fit what the instance is waiting on is rejected
  and leaves the database unchanged. A finished instance accepts no further responses.
- **Application-modelled tables.** The application decides the tables and columns the
  bookmark and the transaction are saved to, so support and reconciliation can query the
  transaction's state directly. The framework decides when saves and loads happen.
- **Auditability.** Every delivered response is journaled with the bookmark it was
  delivered to and the interaction that bookmark was waiting on.
- **Domain-agnostic runtime.** Nothing in the runtime knows about payments; it is generic
  over the flow and over the database.

## 9. Out of scope, deferred

- Typed authorisation on transitions (who may perform which transition), to be added as a
  separate effort. All transitions are currently performed by the system, with the user's
  authority implicit in the responses.
- Subflow composition in the machine form (continuation tags), so that `forced_reversal`
  and `deemed_resolution` are their own machines with their own proofs. The current machine
  is the inlined tree.
- The `#[flow]` macro that lowers the authoring form to the machine form, and the
  `#[persist]` derive that generates the table mapping.
- Other flows on the same entity (collect, mandate, refund, dispute).
- A serialised journal format; the journal currently stores debug text.

## 10. Where each requirement lives

| Requirement | Implementation | Test | Proof (`psp-lean/PspProofs/Pay.lean`) |
|---|---|---|---|
| Entity states and data (§3) | `psp-flow/src/entity.rs` | round trip in `psp-app/tests/persistence.rs` | |
| Transitions only inside the flow (§3) | `Txn<S>` methods, called only from `resume` | | |
| Refined values (§3) | `psp-flow/src/values.rs` | | |
| Interactions co-defined with responses (§4) | `psp-flow/src/events.rs`, `trait Event { type Response }` | | |
| `validate_pay_intent` (§4) | `psp-flow/src/values.rs` | | used in `P6b` |
| `build_req_pay` only from an authenticated transaction (§4) | method on `Txn<Authenticated>` | | `P5_reqpay_only_when_authenticated` |
| The flow, one wait point per `Vis` (§5, §6) | `psp-flow/src/machine.rs`: `At*` structs with `event`/`resume` | demo `psp-app/src/bin/demo.rs` | |
| Seven paths (§6) | | scenarios in tests and demo cover paths 1, 3, 6, 7 | |
| P1 | | | `P1_user_notified` |
| P2 | | | `P2_from_start` |
| P3 | `resume` takes exactly its event's response type | | typing fact; `deliver_sound` for the boundary |
| P4 | | | `P4_terminates` |
| P5 | | | `P5_reqpay_only_when_authenticated` |
| P6a | | | `P6a_id_preserved` |
| P6b | | | `P6b_from_start` |
| Save before wait, load on response (§8) | `driver-framework/src/lib.rs`: `Driver::start`, `Driver::deliver` | `entity_state_column_tracks_the_lifecycle` | |
| Crash recovery (§8) | `Driver::pending` | `crash_recovery_resumes_from_the_tables` | |
| Rollback on mismatch (§8) | `deliver` returns before commit | `violation_rolls_back_and_changes_nothing`, `finished_instances_refuse_more_responses` | |
| Application-modelled tables (§8) | `psp-app/src/persistence.rs`: `impl Persist<Sqlite> for PayFlow` | `every_state_round_trips_through_the_tables` | |
| Auditability (§8) | `store-framework`: `flow_journal` table | journal assertions in tests | |
| Domain-agnostic runtime (§8) | `store-framework`, `driver-framework` | no domain words in either crate | |
| Proofs are about the running code (§7) | `scripts/translate-psp.sh`: Charon → Aeneas → `lake build` | | all theorems; standard axioms only |
