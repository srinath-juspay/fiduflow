//! Drives the persisted pay flow through three scenarios and prints the application's
//! `upi_txn.state` column after every interaction, plus the journal, so the persistence
//! contract is visible: saved before each wait, loaded on each response.

use psp_app::{entity_state, open, Outcome, PayDriver};
use psp_flow::*;

fn show(d: &PayDriver, id: i64, label: &str, o: &Outcome<AnyEvent, Txn<Terminal>>) {
    let state = entity_state(d.backend().connection(), id).unwrap();
    match o {
        Outcome::Wait(ev) => println!("  {label:<34} upi_txn.state={state:<16} waiting on {ev:?}"),
        Outcome::Done(t) => println!("  {label:<34} upi_txn.state={state:<16} DONE {:?}", t.data),
    }
}

fn main() {
    let db = std::env::temp_dir().join(format!("psp-demo-{}.sqlite", std::process::id()));
    let path = db.to_str().unwrap();

    println!("=== 1. happy path (file db: {path})");
    let mut d = open(path).unwrap();
    let (id, ev) = d.start(start(TxnId(1001), PayerId(7))).unwrap();
    println!("  {:<34} upi_txn.state={:<16} waiting on {ev:?}", "start", entity_state(d.backend().connection(), id).unwrap());
    let o = d.deliver(id, AnyResponse::Intent(RawIntent { payee: Vpa(42), amount: Amount(50_000) })).unwrap(); show(&d, id, "User.GetPaymentIntent ->", &o);
    let o = d.deliver(id, AnyResponse::Mpin(Mpin(1234))).unwrap();                                            show(&d, id, "User.CollectMpin ->", &o);
    let o = d.deliver(id, AnyResponse::ReqPay(ReqPayResponse::Ack)).unwrap();                                 show(&d, id, "NPCI.ReqPay -> Ack", &o);
    let o = d.deliver(id, AnyResponse::Callback(Callback::Success { debit_ref: Ref(1), credit_ref: Ref(2) })).unwrap(); show(&d, id, "NPCI.AwaitCallback -> Success", &o);
    let o = d.deliver(id, AnyResponse::Unit).unwrap();                                                        show(&d, id, "User.Notify ->", &o);

    println!("\n=== 2. crash between NPCI ack and callback, then recovery from the database");
    let (id2, _) = d.start(start(TxnId(1002), PayerId(7))).unwrap();
    d.deliver(id2, AnyResponse::Intent(RawIntent { payee: Vpa(42), amount: Amount(75_000) })).unwrap();
    d.deliver(id2, AnyResponse::Mpin(Mpin(1234))).unwrap();
    let o = d.deliver(id2, AnyResponse::ReqPay(ReqPayResponse::Ack)).unwrap(); show(&d, id2, "NPCI.ReqPay -> Ack", &o);
    drop(d);
    println!("  -- process crashed; nothing in memory --");
    let mut d = open(path).unwrap();
    for (pid, ev) in d.pending().unwrap() {
        println!("  recovered instance {pid}: re-issue {ev:?}");
    }
    let o = d.deliver(id2, AnyResponse::Callback(Callback::Deemed)).unwrap();                     show(&d, id2, "NPCI.AwaitCallback -> Deemed", &o);
    let o = d.deliver(id2, AnyResponse::Unit).unwrap();                                            show(&d, id2, "User.Notify ->", &o);
    let o = d.deliver(id2, AnyResponse::Resolution(Resolution::DeadlineExpired)).unwrap();         show(&d, id2, "NPCI.AwaitDeemed -> DeadlineExpired", &o);
    let o = d.deliver(id2, AnyResponse::Unit).unwrap();                                            show(&d, id2, "User.Notify ->", &o);
    let o = d.deliver(id2, AnyResponse::Unit).unwrap();                                            show(&d, id2, "NPCI.AwaitForcedReversal ->", &o);
    let o = d.deliver(id2, AnyResponse::Unit).unwrap();                                            show(&d, id2, "User.Notify ->", &o);
    println!("  journal:");
    for row in d.journal(id2).unwrap() {
        println!("    #{} at {:<60} <- {}", row.seq, row.entry.at, row.entry.response);
    }

    println!("\n=== 3. protocol violation is rejected and rolled back");
    let (id3, _) = d.start(start(TxnId(1003), PayerId(7))).unwrap();
    let before = entity_state(d.backend().connection(), id3).unwrap();
    let err = d.deliver(id3, AnyResponse::Mpin(Mpin(1))).unwrap_err();
    println!("  delivering Mpin to AtGetPaymentIntent: {err:?}; state still {}", entity_state(d.backend().connection(), id3).unwrap());
    assert_eq!(before, entity_state(d.backend().connection(), id3).unwrap());

    let _ = std::fs::remove_file(&db);
}
