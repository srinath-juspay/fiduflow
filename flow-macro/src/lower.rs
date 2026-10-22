//! Lowering of the flow body to the machine: one wait point per `ask(..).await`.
//!
//! The body is walked statement by statement with a typed environment. Every `ask` ends
//! the current wait point's `resume` with `Next::Wait(<next point>)` and starts a new one
//! whose `resume` is the lowering of the rest of the body. A `let x = match ask(..) { .. }`
//! appends the rest of the body to every arm that yields a value, so asks inside arms work
//! naturally; arms that `return` end the run with `Next::Done`.
//!
//! Types are tracked from the events (response types), the entity (transition targets),
//! the decisions (declared signatures) and patterns over those; nothing is inferred from
//! arbitrary Rust. Captured fields of a wait point are exactly the outer locals its
//! `event()` and `resume()` actually use.

use crate::ast::*;
use crate::gen_types::{derives, response_variant};
use crate::rewrite::{pat_idents, Rewriter};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::{Arm, Attribute, Expr, Ident, Pat, Stmt, Type};

// ---------------------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------------------

#[derive(Clone)]
struct Binding {
    name: String,
    /// The binding's own identifier, with its original span. References the generator
    /// emits reuse it, so hygiene-aware tooling resolves them to this binding.
    ident: Ident,
    ty: Type,
    /// A field of the wait point whose `resume` is being generated (`self.name`).
    captured: bool,
}

#[derive(Clone, Default)]
struct Env {
    vars: Vec<Binding>,
}

impl Env {
    fn push(&mut self, name: &Ident, ty: Type, captured: bool) {
        self.vars.push(Binding { name: name.to_string(), ident: name.clone(), ty, captured });
    }
    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.vars.iter().rev().find(|b| b.name == name)
    }
    /// All current bindings marked as captured: the environment seen from inside a new
    /// wait point's `resume`.
    fn all_captured(&self) -> Env {
        let mut e = self.clone();
        for b in &mut e.vars {
            b.captured = true;
        }
        e
    }
    fn captured_names(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for b in &self.vars {
            if self.lookup(&b.name).map(|l| l.captured).unwrap_or(false) {
                out.insert(b.name.clone());
            }
        }
        out
    }
}

enum Known<'a> {
    Handle(&'a StateDef),
    Result(Type, Type),
    Option(Type),
    Enum(&'a [VariantDef]),
    Plain,
}

// ---------------------------------------------------------------------------------------
// Lowerer
// ---------------------------------------------------------------------------------------

struct WaitPoint {
    name: Ident,
    event: Ident,
    fields: Vec<(Ident, Type)>,
    event_body: TokenStream,
    resume_body: TokenStream,
}

pub struct Lowerer<'a> {
    def: &'a FlowDef,
    points: Vec<WaitPoint>,
    used_names: HashSet<String>,
    hit_stack: Vec<HashSet<String>>,
    /// Constructor of the first wait point, as seen from `start`'s environment.
    start_ctor: Option<TokenStream>,
}

pub fn machine_module(def: &FlowDef) -> syn::Result<TokenStream> {
    let mut l = Lowerer { def, points: Vec::new(), used_names: HashSet::new(), hit_stack: vec![HashSet::new()], start_ctor: None };
    l.generate()
}

fn err<T>(span: Span, msg: impl Into<String>) -> syn::Result<T> {
    Err(syn::Error::new(span, msg.into()))
}

fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn single_ident(e: &Expr) -> Option<Ident> {
    if let Expr::Path(p) = e {
        if p.qself.is_none() && p.path.leading_colon.is_none() && p.path.segments.len() == 1 {
            return Some(p.path.segments[0].ident.clone());
        }
    }
    None
}

fn last_segment(p: &syn::Path) -> Ident {
    p.segments.last().expect("path").ident.clone()
}

/// `#[at(Name)]` on a statement names its wait point.
fn at_label(attrs: &[Attribute]) -> syn::Result<Option<Ident>> {
    for a in attrs {
        if a.path().is_ident("at") {
            return Ok(Some(a.parse_args::<Ident>()?));
        }
    }
    Ok(None)
}

fn expr_attrs(e: &Expr) -> &[Attribute] {
    match e {
        Expr::Await(x) => &x.attrs,
        Expr::Match(x) => &x.attrs,
        Expr::MethodCall(x) => &x.attrs,
        Expr::Return(x) => &x.attrs,
        Expr::Call(x) => &x.attrs,
        _ => &[],
    }
}

/// `ask(Party::Event { .. }).await` -> (event name, payload struct literal)
fn as_ask(e: &Expr) -> Option<(Ident, syn::ExprStruct)> {
    let Expr::Await(aw) = e else { return None };
    let Expr::Call(call) = &*aw.base else { return None };
    if single_ident(&call.func).map(|i| i == "ask") != Some(true) || call.args.len() != 1 {
        return None;
    }
    let Expr::Struct(st) = &call.args[0] else { return None };
    let ev = last_segment(&st.path);
    let mut payload = st.clone();
    payload.attrs.clear();
    payload.path = syn::parse_quote!(#ev);
    Some((ev, payload))
}

/// `h.to(State { .. })`
fn as_transition(e: &Expr) -> Option<(Ident, syn::ExprStruct)> {
    let Expr::MethodCall(mc) = e else { return None };
    if mc.method != "to" || mc.args.len() != 1 {
        return None;
    }
    let h = single_ident(&mc.receiver)?;
    let Expr::Struct(st) = &mc.args[0] else { return None };
    Some((h, st.clone()))
}

/// Split an arm body into its statements and its tail value, if any.
fn split_body(e: &Expr) -> (Vec<Stmt>, Option<Expr>) {
    match e {
        Expr::Block(b) => {
            let mut stmts = b.block.stmts.clone();
            match stmts.last() {
                Some(Stmt::Expr(_, None)) => {
                    let Some(Stmt::Expr(tail, None)) = stmts.pop() else { unreachable!() };
                    (stmts, Some(tail))
                }
                _ => (stmts, None),
            }
        }
        other => (Vec::new(), Some(other.clone())),
    }
}

fn ends_with_return(stmts: &[Stmt]) -> bool {
    matches!(stmts.last(), Some(Stmt::Expr(Expr::Return(_), _)))
}

impl<'a> Lowerer<'a> {
    // ---- lookups ------------------------------------------------------------------

    fn state(&self, name: &Ident) -> Option<&'a StateDef> {
        self.def.entity.states.iter().find(|s| &s.name == name)
    }
    fn event(&self, name: &Ident) -> Option<&'a EventDef> {
        self.def.events.iter().find(|e| &e.name == name)
    }
    fn decision(&self, name: &Ident) -> Option<&'a DecisionDef> {
        self.def.decisions.iter().find(|d| &d.name == name)
    }
    fn response_enum(&self, name: &Ident) -> Option<&'a [VariantDef]> {
        self.def.events.iter().find_map(|e| match &e.response {
            ResponseDef::Enum { name: n, variants } if n == name => Some(variants.as_slice()),
            _ => None,
        })
    }

    fn handle_type(&self, state: &Ident) -> Type {
        let w = &self.def.entity.name;
        syn::parse_quote!(#w<#state>)
    }

    fn classify(&self, ty: &Type) -> Known<'a> {
        let Type::Path(tp) = ty else { return Known::Plain };
        let Some(seg) = tp.path.segments.last() else { return Known::Plain };
        let args: Vec<&Type> = match &seg.arguments {
            syn::PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|g| if let syn::GenericArgument::Type(t) = g { Some(t) } else { None })
                .collect(),
            _ => Vec::new(),
        };
        if seg.ident == self.def.entity.name && args.len() == 1 {
            if let Type::Path(sp) = args[0] {
                if let Some(st) = self.state(&last_segment(&sp.path)) {
                    return Known::Handle(st);
                }
            }
            return Known::Plain;
        }
        if seg.ident == "Result" && args.len() == 2 {
            return Known::Result(args[0].clone(), args[1].clone());
        }
        if seg.ident == "Option" && args.len() == 1 {
            return Known::Option(args[0].clone());
        }
        if let Some(vs) = self.response_enum(&seg.ident) {
            return Known::Enum(vs);
        }
        Known::Plain
    }

    fn handle_fields(&self, env: &Env) -> HashMap<String, HashSet<String>> {
        let mut out = HashMap::new();
        for b in &env.vars {
            if let Known::Handle(st) = self.classify(&b.ty) {
                out.insert(b.name.clone(), st.fields.iter().map(|f| f.name.to_string()).collect());
            }
        }
        out
    }

    fn fresh(&mut self, base: Ident) -> Ident {
        let mut name = base.to_string();
        let mut n = 2;
        while self.used_names.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        self.used_names.insert(name.clone());
        Ident::new(&name, base.span())
    }

    // ---- typing -------------------------------------------------------------------

    fn type_of(&self, e: &Expr, env: &Env) -> syn::Result<Type> {
        if let Some((ev, _)) = as_ask(e) {
            let Some(evd) = self.event(&ev) else { return err(ev.span(), format!("unknown event `{ev}`")) };
            return Ok(evd.response_type());
        }
        if let Some((_, st)) = as_transition(e) {
            let target = last_segment(&st.path);
            if self.state(&target).is_none() {
                return err(target.span(), format!("`{target}` is not a state of the entity"));
            }
            return Ok(self.handle_type(&target));
        }
        match e {
            Expr::Call(c) => {
                if let Some(f) = single_ident(&c.func) {
                    if let Some(d) = self.decision(&f) {
                        return Ok(d.output.clone());
                    }
                    return err(f.span(), format!("`{f}` is not a declared decision; only decisions may be called in a flow body"));
                }
                err(c.paren_token.span.join(), "unsupported call in flow body")
            }
            Expr::Path(_) => {
                let id = single_ident(e).ok_or_else(|| syn::Error::new(Span::call_site(), "unsupported path"))?;
                env.lookup(&id.to_string())
                    .map(|b| b.ty.clone())
                    .ok_or_else(|| syn::Error::new(id.span(), format!("unknown variable `{id}`")))
            }
            Expr::Match(m) => {
                let sty = self.type_of(&m.expr, env)?;
                for arm in &m.arms {
                    let (_, tail) = split_body(&arm.body);
                    if let Some(t) = tail {
                        if matches!(t, Expr::Return(_)) {
                            continue;
                        }
                        let mut env2 = env.clone();
                        self.bind_pattern(&arm.pat, &sty, &mut env2)?;
                        return self.type_of(&t, &env2);
                    }
                }
                err(m.match_token.span, "cannot type a match whose every arm returns")
            }
            Expr::Block(b) => match b.block.stmts.last() {
                Some(Stmt::Expr(t, None)) => self.type_of(t, env),
                _ => err(b.block.brace_token.span.join(), "block has no tail expression"),
            },
            Expr::Paren(p) => self.type_of(&p.expr, env),
            Expr::Reference(r) => {
                let inner = self.type_of(&r.expr, env)?;
                Ok(syn::parse_quote!(&#inner))
            }
            Expr::Field(f) => {
                if let (Some(base), syn::Member::Named(m)) = (single_ident(&f.base), &f.member) {
                    if let Some(b) = env.lookup(&base.to_string()) {
                        if let Known::Handle(st) = self.classify(&b.ty) {
                            if m == "id" {
                                return Ok(self.def.entity.id_ty.clone());
                            }
                            if let Some(fd) = st.fields.iter().find(|fd| &fd.name == m) {
                                return Ok(fd.ty.clone());
                            }
                        }
                    }
                }
                err(f.dot_token.span, "cannot type this field access; add a type annotation to the binding")
            }
            _ => err(Span::call_site(), "cannot infer the type of this expression; add a type annotation to the binding (`let x: T = ..`)"),
        }
    }

    fn bind_pattern(&self, pat: &Pat, ty: &Type, env: &mut Env) -> syn::Result<()> {
        match pat {
            Pat::Ident(pi) => {
                env.push(&pi.ident, ty.clone(), false);
                Ok(())
            }
            Pat::Wild(_) | Pat::Path(_) | Pat::Lit(_) => Ok(()),
            Pat::Type(pt) => self.bind_pattern(&pt.pat, &pt.ty, env),
            Pat::Paren(p) => self.bind_pattern(&p.pat, ty, env),
            Pat::Reference(r) => self.bind_pattern(&r.pat, ty, env),
            Pat::Or(o) => {
                let first = o.cases.first().ok_or_else(|| syn::Error::new(o.cases.span(), "empty or-pattern"))?;
                self.bind_pattern(first, ty, env)
            }
            Pat::TupleStruct(ts) => {
                let v = last_segment(&ts.path);
                let inner: Option<Type> = match (v.to_string().as_str(), self.classify(ty)) {
                    ("Ok", Known::Result(a, _)) => Some(a),
                    ("Err", Known::Result(_, b)) => Some(b),
                    ("Some", Known::Option(a)) => Some(a),
                    _ => None,
                };
                match (inner, ts.elems.len()) {
                    (Some(t), 1) => self.bind_pattern(&ts.elems[0], &t, env),
                    _ => err(v.span(), format!("cannot type pattern `{v}(..)` against the scrutinee type")),
                }
            }
            Pat::Struct(ps) => {
                let v = last_segment(&ps.path);
                let Known::Enum(variants) = self.classify(ty) else {
                    return err(v.span(), "struct pattern on a type that is not a declared response enum");
                };
                let Some(vd) = variants.iter().find(|x| x.name == v) else {
                    return err(v.span(), format!("`{v}` is not a variant of the response enum"));
                };
                for fp in &ps.fields {
                    let syn::Member::Named(fname) = &fp.member else { continue };
                    let Some(fd) = vd.fields.iter().find(|f| &f.name == fname) else {
                        return err(fname.span(), format!("variant `{v}` has no field `{fname}`"));
                    };
                    self.bind_pattern(&fp.pat, &fd.ty, env)?;
                }
                Ok(())
            }
            other => {
                let mut ids = Vec::new();
                pat_idents(other, &mut ids);
                if ids.is_empty() {
                    Ok(())
                } else {
                    err(ids[0].span(), "unsupported pattern in flow body")
                }
            }
        }
    }

    // ---- expression expansion -------------------------------------------------------

    /// Expand `h.to(State { .. })` into the wrapper struct literal. `terminal_wrap` wraps
    /// a terminal target in `Next::Done(..)`; otherwise a terminal target is an error.
    fn expand_transition(&self, h: &Ident, st: &syn::ExprStruct, env: &Env, terminal_wrap: bool) -> syn::Result<Expr> {
        let Some(b) = env.lookup(&h.to_string()) else { return err(h.span(), format!("unknown handle `{h}`")) };
        let Known::Handle(src) = self.classify(&b.ty) else {
            return err(h.span(), format!("`{h}` is not an entity handle"));
        };
        let target_name = last_segment(&st.path);
        let Some(target) = self.state(&target_name) else {
            return err(target_name.span(), format!("`{target_name}` is not a state of the entity"));
        };
        if target.terminal && !terminal_wrap {
            return err(target_name.span(), format!("`{target_name}` is terminal: a transition to it must be a `return` or the tail of the body"));
        }
        if !target.terminal && terminal_wrap {
            return err(target_name.span(), format!("`{target_name}` is not terminal: a `return` must end in a terminal state"));
        }
        let mut given: HashMap<String, Option<Expr>> = HashMap::new();
        for fv in &st.fields {
            let syn::Member::Named(n) = &fv.member else { continue };
            let explicit = if fv.colon_token.is_some() { Some(fv.expr.clone()) } else { None };
            given.insert(n.to_string(), explicit);
        }
        let mut inits = Vec::new();
        for f in &target.fields {
            let fname = &f.name;
            let key = fname.to_string();
            let Some(explicit) = given.remove(&key) else {
                return err(target_name.span(), format!("transition to `{target_name}` is missing field `{fname}`"));
            };
            let value: Expr = match explicit {
                Some(e) => e,
                None => {
                    if src.fields.iter().any(|sf| sf.name == *fname) {
                        syn::parse_quote!(#h.data.#fname)
                    } else if env.lookup(&key).is_some() {
                        syn::parse_quote!(#fname)
                    } else {
                        return err(fname.span(), format!("`{fname}` is neither a field of `{}` nor a variable in scope", src.name));
                    }
                }
            };
            inits.push(quote! { #fname: #value });
        }
        if let Some((extra, _)) = given.into_iter().next() {
            return err(target_name.span(), format!("`{target_name}` has no field `{extra}`"));
        }
        let w = &self.def.entity.name;
        let lit: Expr = if target.fields.is_empty() {
            syn::parse_quote!(#target_name)
        } else {
            syn::parse_quote!(#target_name { #(#inits),* })
        };
        Ok(if terminal_wrap {
            syn::parse_quote!(Next::Done(#w { id: #h.id, data: Terminal::#target_name(#lit) }))
        } else {
            syn::parse_quote!(#w { id: #h.id, data: #lit })
        })
    }

    /// Expand transitions inside an expression (anywhere a method call `.to` appears).
    fn expand_all_transitions(&self, e: &mut Expr, env: &Env, terminal_wrap: bool) -> syn::Result<()> {
        struct V<'l, 'a> {
            l: &'l Lowerer<'a>,
            env: &'l Env,
            terminal_wrap: bool,
            error: Option<syn::Error>,
        }
        impl syn::visit_mut::VisitMut for V<'_, '_> {
            fn visit_expr_mut(&mut self, e: &mut Expr) {
                if let Some((h, st)) = as_transition(e) {
                    match self.l.expand_transition(&h, &st, self.env, self.terminal_wrap) {
                        Ok(x) => *e = x,
                        Err(er) => {
                            if self.error.is_none() {
                                self.error = Some(er);
                            }
                        }
                    }
                    return;
                }
                syn::visit_mut::visit_expr_mut(self, e);
            }
        }
        let mut v = V { l: self, env, terminal_wrap, error: None };
        syn::visit_mut::VisitMut::visit_expr_mut(&mut v, e);
        match v.error {
            Some(er) => Err(er),
            None => Ok(()),
        }
    }

    /// Expand transitions, then rewrite captured locals and handle sugar.
    fn emit_expr(&mut self, e: &Expr, env: &Env, terminal_wrap: bool) -> syn::Result<TokenStream> {
        let mut e = e.clone();
        self.expand_all_transitions(&mut e, env, terminal_wrap)?;
        let captured = env.captured_names();
        let hf = self.handle_fields(env);
        let mut rw = Rewriter::new(&captured, &hf);
        rw.rewrite_expr(&mut e);
        let hits = rw.hits;
        self.hit_stack.last_mut().expect("hit stack").extend(hits);
        Ok(quote! { #e })
    }

    // ---- lowering -------------------------------------------------------------------

    /// Lower a statement list to an expression of type `Next`.
    fn lower(&mut self, stmts: Vec<Stmt>, env: Env) -> syn::Result<TokenStream> {
        let mut it = stmts.into_iter();
        let Some(first) = it.next() else {
            return err(Span::call_site(), "flow body must end with a `return` or a transition to a terminal state");
        };
        let rest: Vec<Stmt> = it.collect();

        match first {
            Stmt::Local(l) => {
                let label = at_label(&l.attrs)?;
                let Some(init) = &l.init else { return err(l.let_token.span, "every `let` in a flow body needs an initialiser") };
                let init = (*init.expr).clone();
                let pat = l.pat.clone();

                if let Some((ev, payload)) = as_ask(&init) {
                    let evd = self.event_def(&ev)?;
                    let resp_ty = evd.response_type();
                    let ctor = self.wait_point(label, evd, &payload, &env, |me, env2| {
                        let mut env3 = env2.clone();
                        me.bind_pattern(&pat, &resp_ty, &mut env3)?;
                        let cont = me.lower(rest.clone(), env3)?;
                        Ok(quote! { let #pat = r; #cont })
                    })?;
                    return Ok(quote! { Next::Wait(#ctor) });
                }

                if let Expr::Match(m) = &init {
                    if let Some((ev, payload)) = as_ask(&m.expr) {
                        let evd = self.event_def(&ev)?;
                        let resp_ty = evd.response_type();
                        let arms = m.arms.clone();
                        let ctor = self.wait_point(label, evd, &payload, &env, |me, env2| {
                            let lowered = me.lower_arms(&arms, &resp_ty, env2, Some(&pat), &rest)?;
                            Ok(quote! { match r { #(#lowered)* } })
                        })?;
                        return Ok(quote! { Next::Wait(#ctor) });
                    }
                    // pure scrutinee
                    let sty = self.type_of(&m.expr, &env)?;
                    let scrut = self.emit_expr(&m.expr, &env, false)?;
                    let lowered = self.lower_arms(&m.arms, &sty, &env, Some(&pat), &rest)?;
                    return Ok(quote! { match #scrut { #(#lowered)* } });
                }

                // pure `let`
                let ty = match &pat {
                    Pat::Type(pt) => (*pt.ty).clone(),
                    _ => self.type_of(&init, &env)?,
                };
                let value = self.emit_expr(&init, &env, false)?;
                let mut env2 = env.clone();
                self.bind_pattern(&pat, &ty, &mut env2)?;
                let cont = self.lower(rest, env2)?;
                Ok(quote! { { let #pat = #value; #cont } })
            }

            Stmt::Expr(e, semi) => {
                let label = at_label(expr_attrs(&e))?;
                if let Some((ev, payload)) = as_ask(&e) {
                    let evd = self.event_def(&ev)?;
                    let ctor = self.wait_point(label, evd, &payload, &env, |me, env2| {
                        let cont = me.lower(rest.clone(), env2.clone())?;
                        Ok(quote! { let _ = r; #cont })
                    })?;
                    return Ok(quote! { Next::Wait(#ctor) });
                }
                match &e {
                    Expr::Return(r) => {
                        let Some(val) = &r.expr else { return err(r.return_token.span, "`return` in a flow body must carry a terminal transition") };
                        self.emit_expr(val, &env, true)
                    }
                    Expr::Match(m) => {
                        if let Some((ev, payload)) = as_ask(&m.expr) {
                            let evd = self.event_def(&ev)?;
                            let resp_ty = evd.response_type();
                            let arms = m.arms.clone();
                            let ctor = self.wait_point(label, evd, &payload, &env, |me, env2| {
                                let lowered = me.lower_arms(&arms, &resp_ty, env2, None, &rest)?;
                                Ok(quote! { match r { #(#lowered)* } })
                            })?;
                            return Ok(quote! { Next::Wait(#ctor) });
                        }
                        let sty = self.type_of(&m.expr, &env)?;
                        let scrut = self.emit_expr(&m.expr, &env, false)?;
                        let lowered = self.lower_arms(&m.arms, &sty, &env, None, &rest)?;
                        Ok(quote! { match #scrut { #(#lowered)* } })
                    }
                    _ if semi.is_none() && rest.is_empty() => {
                        // tail: must be a terminal transition
                        if as_transition(&e).is_none() {
                            return err(Span::call_site(), "the tail of a flow body must be a transition to a terminal state");
                        }
                        self.emit_expr(&e, &env, true)
                    }
                    _ => err(Span::call_site(), "unsupported statement in flow body: only `let`, `match`, `ask(..).await`, `return` and a terminal transition are allowed"),
                }
            }
            _ => err(Span::call_site(), "unsupported item in flow body"),
        }
    }

    fn event_def(&self, ev: &Ident) -> syn::Result<&'a EventDef> {
        self.event(ev).ok_or_else(|| syn::Error::new(ev.span(), format!("unknown event `{ev}`")))
    }

    /// Lower match arms. With `cont = Some(pat)`, every arm that yields a value binds it
    /// to `pat` and continues with `rest`; arms that `return` end the run.
    fn lower_arms(&mut self, arms: &[Arm], scrutinee_ty: &Type, env: &Env, cont: Option<&Pat>, rest: &[Stmt]) -> syn::Result<Vec<TokenStream>> {
        let mut out = Vec::new();
        for arm in arms {
            if let Some((if_tok, _)) = &arm.guard {
                return err(if_tok.span, "match guards are not supported in a flow body");
            }
            let mut env2 = env.clone();
            self.bind_pattern(&arm.pat, scrutinee_ty, &mut env2)?;
            let (mut stmts, tail) = split_body(&arm.body);
            match (tail, cont) {
                (Some(Expr::Return(r)), _) => stmts.push(Stmt::Expr(Expr::Return(r), None)),
                (Some(t), Some(pat)) => {
                    stmts.push(syn::parse_quote!(let #pat = #t;));
                    stmts.extend(rest.iter().cloned());
                }
                (Some(t), None) => {
                    stmts.push(Stmt::Expr(t, None));
                    stmts.extend(rest.iter().cloned());
                }
                (None, _) => {
                    if !ends_with_return(&stmts) {
                        return err(arm.fat_arrow_token.spans[0], "a match arm must yield a value or `return`");
                    }
                }
            }
            let body = self.lower(stmts, env2)?;
            let pat = &arm.pat;
            out.push(quote! { #pat => { #body } });
        }
        Ok(out)
    }

    /// Create a wait point for `evd` with payload `payload`. `build` produces the resume
    /// body given the environment seen from inside it. Returns the constructor expression
    /// `Flow::Name(Name { fields })`, rewritten for the current environment.
    fn wait_point(
        &mut self,
        label: Option<Ident>,
        evd: &'a EventDef,
        payload: &syn::ExprStruct,
        env: &Env,
        build: impl FnOnce(&mut Self, &Env) -> syn::Result<TokenStream>,
    ) -> syn::Result<TokenStream> {
        let name = self.fresh(label.unwrap_or_else(|| format_ident!("At{}", evd.name)));
        let inner_env = env.all_captured();

        // Reserve the slot now so wait points are emitted in flow order (outer before the
        // points reached from inside its resume), then fill it in below.
        let slot = self.points.len();
        self.points.push(WaitPoint { name: name.clone(), event: evd.name.clone(), fields: Vec::new(), event_body: quote! {}, resume_body: quote! {} });

        self.hit_stack.push(HashSet::new());
        let payload_expr: Expr = Expr::Struct(payload.clone());
        let event_body = self.emit_expr(&payload_expr, &inner_env, false)?;
        let resume_body = build(self, &inner_env)?;
        let hits = self.hit_stack.pop().expect("hit stack");

        // Fields: the hit names, in binding order, with their current types. The field
        // name is fresh; the value is the binding's own identifier (its span included).
        let mut fields: Vec<(Ident, Type)> = Vec::new();
        let mut values: Vec<Ident> = Vec::new();
        let mut seen = HashSet::new();
        for b in &env.vars {
            if hits.contains(&b.name) && seen.insert(b.name.clone()) {
                let latest = env.lookup(&b.name).expect("binding");
                fields.push((Ident::new(&b.name, Span::call_site()), latest.ty.clone()));
                values.push(latest.ident.clone());
            }
        }

        let flow_enum = self.flow_enum();
        let fnames: Vec<&Ident> = fields.iter().map(|(n, _)| n).collect();
        let ctor: Expr = if fields.is_empty() {
            syn::parse_quote!(#flow_enum::#name(#name))
        } else {
            syn::parse_quote!(#flow_enum::#name(#name { #(#fnames: #values),* }))
        };
        self.points[slot] = WaitPoint { name, event: evd.name.clone(), fields, event_body, resume_body };
        // Rewrite the constructor for the *outer* environment (records outer hits).
        let ctor = self.emit_expr(&ctor, env, false)?;
        if slot == 0 {
            self.start_ctor = Some(ctor.clone());
        }
        Ok(ctor)
    }

    fn flow_enum(&self) -> Ident {
        Ident::new(&camel(&self.def.name.to_string()), self.def.name.span())
    }

    // ---- assembly -------------------------------------------------------------------

    fn generate(&mut self) -> syn::Result<TokenStream> {
        let def = self.def;
        let flow_enum = self.flow_enum();
        let param = &def.body_param;
        let input = &def.input;
        let output = &def.output;

        // Initial segment: pure lets, then the first ask.
        let mut env = Env::default();
        env.push(param, input.clone(), false);
        let mut prelude = Vec::new();
        let mut stmts = def.body.stmts.clone();
        let first_ask_idx = stmts.iter().position(|s| match s {
            Stmt::Local(l) => l.init.as_ref().map(|i| as_ask(&i.expr).is_some() || matches!(&*i.expr, Expr::Match(m) if as_ask(&m.expr).is_some())).unwrap_or(false),
            Stmt::Expr(e, _) => as_ask(e).is_some() || matches!(e, Expr::Match(m) if as_ask(&m.expr).is_some()),
            _ => false,
        });
        let Some(idx) = first_ask_idx else {
            return err(def.body.brace_token.span.join(), "a flow body must contain at least one `ask(..).await`");
        };
        let tail: Vec<Stmt> = stmts.split_off(idx);
        for s in stmts {
            let Stmt::Local(l) = s else { return err(Span::call_site(), "only `let` statements may precede the first ask") };
            let Some(init) = &l.init else { return err(l.let_token.span, "`let` needs an initialiser") };
            let ty = match &l.pat {
                Pat::Type(pt) => (*pt.ty).clone(),
                _ => self.type_of(&init.expr, &env)?,
            };
            let value = self.emit_expr(&init.expr, &env, false)?;
            let pat = &l.pat;
            prelude.push(quote! { let #pat = #value; });
            self.bind_pattern(&l.pat, &ty, &mut env)?;
        }
        let _ = self.lower(tail, env)?;
        // The first wait point created is the entry point; `start` returns its constructor
        // directly, so there is no unreachable `Done` arm and no panic path.
        let start_ctor = self.start_ctor.clone().expect("a flow has at least one wait point");
        let start_body = quote! {
            #(#prelude)*
            #start_ctor
        };

        let d = derives();
        let point_structs = self.points.iter().map(|p| {
            let n = &p.name;
            if p.fields.is_empty() {
                quote! { #d pub struct #n; }
            } else {
                let fns = p.fields.iter().map(|(f, _)| f);
                let fts = p.fields.iter().map(|(_, t)| t);
                quote! { #d pub struct #n { #(pub #fns: #fts),* } }
            }
        });
        let point_impls = self.points.iter().map(|p| {
            let n = &p.name;
            let ev = &p.event;
            let eb = &p.event_body;
            let rb = &p.resume_body;
            let flow_enum = &flow_enum;
            let output = &output;
            quote! {
                impl WaitPoint<#flow_enum, #output> for #n {
                    type Event = #ev;
                    fn event(&self) -> #ev { #eb }
                    fn resume(self, r: <#ev as Event>::Response) -> Next { #rb }
                }
            }
        });
        let variants = self.points.iter().map(|p| {
            let n = &p.name;
            quote! { #n(#n) }
        });
        let event_arms = self.points.iter().map(|p| {
            let n = &p.name;
            let ev = &p.event;
            quote! { #flow_enum::#n(w) => AnyEvent::#ev(w.event()) }
        });
        let deliver_arms = self.points.iter().map(|p| {
            let n = &p.name;
            let evd = self.event(&p.event).expect("event");
            match response_variant(&evd.response_type()) {
                None => quote! { (#flow_enum::#n(w), AnyResponse::Unit) => w.resume(()) },
                Some(v) => quote! { (#flow_enum::#n(w), AnyResponse::#v(x)) => w.resume(x) },
            }
        });
        let decisions = def.decisions.iter().map(|dc| {
            let n = &dc.name;
            let ins = &dc.inputs;
            let out = &dc.output;
            let body = &dc.body;
            quote! { pub fn #n(#(#ins),*) -> #out #body }
        });

        Ok(quote! {
            /// The flow as a machine: one typed wait point per `ask`, the bookmark enum, and
            /// the runtime boundary. Generated by `flow!`.
            pub mod machine {
                use super::*;

                #(#decisions)*

                #(#point_structs)*

                /// The bookmark: one variant per wait point.
                #d
                pub enum #flow_enum { #(#variants),* }

                /// The framework's `Next`, instantiated for this flow.
                pub type Next = driver_framework::Next<#flow_enum, #output>;
                pub use driver_framework::WaitPoint;

                /// The response did not decode as the bookmark's event expects.
                #d
                pub enum Mismatch { ResponseMismatch }

                pub fn start(#param: #input) -> #flow_enum { #start_body }

                #(#point_impls)*

                /// What a bookmark is waiting on, as data.
                pub fn event(s: &#flow_enum) -> AnyEvent {
                    match s { #(#event_arms),* }
                }

                /// Decode the response for the bookmark's event and resume. The only place a
                /// mismatch can occur; inside `resume` the types make it impossible.
                pub fn deliver(s: #flow_enum, r: AnyResponse) -> Result<Next, Mismatch> {
                    Ok(match (s, r) {
                        #(#deliver_arms,)*
                        _ => return Err(Mismatch::ResponseMismatch),
                    })
                }
            }
        })
    }
}

use syn::spanned::Spanned;
