//! `flow! { ... }`: one invocation holding a flow's entity (states), its interactions
//! (each co-defined with its response type), its pure decisions, and its body written in
//! an async style. The macro lowers the body to a first-order machine: one struct per
//! `ask(..).await` carrying the locals live at that point, each with `event()` and a typed
//! `resume(r)`, plus the bookmark enum, `start`, and the boundary `event`/`deliver`.
//!
//! The generated code is plain Rust: no closures, no `dyn`, no `async`, so Charon and
//! Aeneas can translate it and proofs are about what runs. It targets the vocabulary in
//! `driver_framework::machine` (`Event`, `WaitPoint`, `Next`), which the invoking crate
//! must depend on.
//!
//! Set `FLOW_MACRO_DUMP=<path>` to write the expansion to a file for inspection.

mod ast;
mod gen_types;
mod lower;
mod rewrite;

use proc_macro::TokenStream;

#[proc_macro]
pub fn flow(input: TokenStream) -> TokenStream {
    let def = syn::parse_macro_input!(input as ast::FlowDef);
    match expand(&def) {
        Ok(ts) => {
            if let Ok(path) = std::env::var("FLOW_MACRO_DUMP") {
                let _ = std::fs::write(path, ts.to_string());
            }
            ts.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(def: &ast::FlowDef) -> syn::Result<proc_macro2::TokenStream> {
    let entity = gen_types::entity_module(def);
    let events = gen_types::events_module(def);
    let machine = lower::machine_module(def)?;
    Ok(quote::quote! {
        #entity
        #events
        #machine
        pub use entity::*;
        pub use events::*;
        pub use machine::*;
    })
}
