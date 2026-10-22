//! Identifier analysis and rewriting over `syn` expressions.
//!
//! `Rewriter` turns captured locals into `self.<name>` inside a generated `resume`, and
//!   expands the handle sugar `h.field` to `h.data.field`, with correct scoping for
//!   `let` bindings and match-arm patterns that shadow a captured name.

use std::collections::{HashMap, HashSet};
use syn::visit_mut::VisitMut;
use syn::{Expr, Ident, Pat, Stmt};

/// Identifiers bound by a pattern.
pub fn pat_idents(p: &Pat, out: &mut Vec<Ident>) {
    match p {
        Pat::Ident(pi) => {
            out.push(pi.ident.clone());
            if let Some((_, sub)) = &pi.subpat {
                pat_idents(sub, out);
            }
        }
        Pat::Struct(ps) => {
            for f in &ps.fields {
                pat_idents(&f.pat, out);
            }
        }
        Pat::TupleStruct(ts) => {
            for e in &ts.elems {
                pat_idents(e, out);
            }
        }
        Pat::Tuple(t) => {
            for e in &t.elems {
                pat_idents(e, out);
            }
        }
        Pat::Or(o) => {
            if let Some(first) = o.cases.first() {
                pat_idents(first, out);
            }
        }
        Pat::Reference(r) => pat_idents(&r.pat, out),
        Pat::Type(t) => pat_idents(&t.pat, out),
        Pat::Paren(p) => pat_idents(&p.pat, out),
        _ => {}
    }
}

pub struct Rewriter<'a> {
    /// Locals that are fields of the wait point being generated: `x` becomes `self.x`.
    pub captured: &'a HashSet<String>,
    /// Entity handles in scope and the fields of their current state: `h.f` becomes `h.data.f`.
    pub handle_fields: &'a HashMap<String, HashSet<String>>,
    /// Captured names that were actually rewritten: the fields the wait point needs.
    pub hits: HashSet<String>,
    scopes: Vec<HashSet<String>>,
}

impl<'a> Rewriter<'a> {
    pub fn new(captured: &'a HashSet<String>, handle_fields: &'a HashMap<String, HashSet<String>>) -> Self {
        Rewriter { captured, handle_fields, hits: HashSet::new(), scopes: vec![HashSet::new()] }
    }

    fn shadowed(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(name))
    }

    fn bind(&mut self, p: &Pat) {
        let mut ids = Vec::new();
        pat_idents(p, &mut ids);
        let top = self.scopes.last_mut().expect("scope");
        for i in ids {
            top.insert(i.to_string());
        }
    }

    fn single_ident(e: &Expr) -> Option<String> {
        if let Expr::Path(p) = e {
            if p.qself.is_none() && p.path.leading_colon.is_none() && p.path.segments.len() == 1 {
                return Some(p.path.segments[0].ident.to_string());
            }
        }
        None
    }

    pub fn rewrite_expr(&mut self, e: &mut Expr) {
        self.visit_expr_mut(e);
    }

}

impl VisitMut for Rewriter<'_> {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        // `h.field` on an entity handle -> `h.data.field` (unless it is `id` or `data`).
        if let Expr::Field(f) = e {
            if let (Some(base), syn::Member::Named(m)) = (Self::single_ident(&f.base), &f.member) {
                let mname = m.to_string();
                if let Some(fields) = self.handle_fields.get(&base) {
                    if !self.shadowed(&base) && fields.contains(&mname) && mname != "id" && mname != "data" {
                        let inner = f.base.clone();
                        f.base = Box::new(syn::parse_quote!(#inner.data));
                    }
                }
            }
        }
        // captured local -> self.local
        if let Some(name) = Self::single_ident(e) {
            if self.captured.contains(&name) && !self.shadowed(&name) {
                self.hits.insert(name.clone());
                let id = syn::Ident::new(&name, proc_macro2::Span::call_site());
                *e = syn::parse_quote!(self.#id);
                return;
            }
        }
        match e {
            Expr::Match(m) => {
                self.visit_expr_mut(&mut m.expr);
                for arm in &mut m.arms {
                    self.scopes.push(HashSet::new());
                    self.bind(&arm.pat);
                    if let Some((_, guard)) = &mut arm.guard {
                        self.visit_expr_mut(guard);
                    }
                    self.visit_expr_mut(&mut arm.body);
                    self.scopes.pop();
                }
            }
            Expr::Closure(_) => {
                // Closures are not part of the flow language; leave untouched.
            }
            _ => syn::visit_mut::visit_expr_mut(self, e),
        }
    }

    fn visit_block_mut(&mut self, b: &mut syn::Block) {
        self.scopes.push(HashSet::new());
        for s in &mut b.stmts {
            self.visit_stmt_mut(s);
        }
        self.scopes.pop();
    }

    fn visit_stmt_mut(&mut self, s: &mut Stmt) {
        match s {
            Stmt::Local(l) => {
                if let Some(init) = &mut l.init {
                    self.visit_expr_mut(&mut init.expr);
                    if let Some((_, diverge)) = &mut init.diverge {
                        self.visit_expr_mut(diverge);
                    }
                }
                let pat = l.pat.clone();
                self.bind(&pat);
            }
            _ => syn::visit_mut::visit_stmt_mut(self, s),
        }
    }

    fn visit_field_value_mut(&mut self, fv: &mut syn::FieldValue) {
        self.visit_expr_mut(&mut fv.expr);
        // A shorthand `Struct { x }` whose `x` was rewritten must print `x: self.x`.
        if fv.colon_token.is_none() {
            fv.colon_token = Some(Default::default());
        }
    }
}
