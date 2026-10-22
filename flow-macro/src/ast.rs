//! The grammar of a `flow!` block and its parser.
//!
//! ```text
//! flow! {
//!     <name>: <InputType> -> <OutputType>;
//!     entity <Wrapper> (id: <IdType>) {
//!         [terminal] <State> { <field>: <Type>, ... }
//!         ...
//!     }
//!     events {
//!         <Party>.<Event> { <field>: <Type>, ... } -> <ResponseType> ;
//!         <Party>.<Event> { ... } -> enum <Name> { <Variant> [{ fields }], ... } ;
//!     }
//!     decision <name>(<params>) -> <Type> { <rust> }
//!     body(<handle>) { <restricted rust> }
//! }
//! ```

use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{braced, parenthesized, Block, FnArg, Ident, Token, Type};

pub struct FlowDef {
    pub name: Ident,
    pub input: Type,
    pub output: Type,
    pub entity: EntityDef,
    pub events: Vec<EventDef>,
    pub decisions: Vec<DecisionDef>,
    pub body_param: Ident,
    pub body: Block,
}

pub struct EntityDef {
    pub name: Ident,
    pub id_ty: Type,
    pub states: Vec<StateDef>,
}

pub struct StateDef {
    pub name: Ident,
    pub terminal: bool,
    pub fields: Vec<FieldDef>,
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
}

pub struct EventDef {
    pub party: Ident,
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub response: ResponseDef,
}

pub enum ResponseDef {
    Ty(Type),
    Enum { name: Ident, variants: Vec<VariantDef> },
}

pub struct VariantDef {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
}

pub struct DecisionDef {
    pub name: Ident,
    pub inputs: Vec<FnArg>,
    pub output: Type,
    pub body: Block,
}

impl EventDef {
    /// The response type as written in the generated code.
    pub fn response_type(&self) -> Type {
        match &self.response {
            ResponseDef::Ty(t) => t.clone(),
            ResponseDef::Enum { name, .. } => syn::parse_quote!(#name),
        }
    }
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(FieldDef { name, ty })
    }
}

fn parse_fields(input: ParseStream) -> syn::Result<Vec<FieldDef>> {
    let content;
    braced!(content in input);
    let fields: Punctuated<FieldDef, Token![,]> = content.parse_terminated(FieldDef::parse, Token![,])?;
    Ok(fields.into_iter().collect())
}

impl Parse for StateDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let terminal = if input.peek(Ident) && input.fork().parse::<Ident>()? == "terminal" {
            input.parse::<Ident>()?;
            true
        } else {
            false
        };
        let name: Ident = input.parse()?;
        let fields = if input.peek(syn::token::Brace) { parse_fields(input)? } else { Vec::new() };
        Ok(StateDef { name, terminal, fields })
    }
}

impl Parse for EntityDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let paren;
        parenthesized!(paren in input);
        let id_kw: Ident = paren.parse()?;
        if id_kw != "id" {
            return Err(syn::Error::new(id_kw.span(), "expected `id: <Type>`"));
        }
        paren.parse::<Token![:]>()?;
        let id_ty: Type = paren.parse()?;
        let content;
        braced!(content in input);
        let mut states = Vec::new();
        while !content.is_empty() {
            states.push(content.parse()?);
        }
        Ok(EntityDef { name, id_ty, states })
    }
}

impl Parse for VariantDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let fields = if input.peek(syn::token::Brace) { parse_fields(input)? } else { Vec::new() };
        Ok(VariantDef { name, fields })
    }
}

impl Parse for EventDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let party: Ident = input.parse()?;
        input.parse::<Token![.]>()?;
        let name: Ident = input.parse()?;
        let fields = if input.peek(syn::token::Brace) { parse_fields(input)? } else { Vec::new() };
        input.parse::<Token![->]>()?;
        let response = if input.peek(Token![enum]) {
            input.parse::<Token![enum]>()?;
            let ename: Ident = input.parse()?;
            let content;
            braced!(content in input);
            let variants: Punctuated<VariantDef, Token![,]> = content.parse_terminated(VariantDef::parse, Token![,])?;
            ResponseDef::Enum { name: ename, variants: variants.into_iter().collect() }
        } else {
            ResponseDef::Ty(input.parse()?)
        };
        input.parse::<Token![;]>()?;
        Ok(EventDef { party, name, fields, response })
    }
}

impl Parse for DecisionDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let paren;
        parenthesized!(paren in input);
        let inputs: Punctuated<FnArg, Token![,]> = paren.parse_terminated(FnArg::parse, Token![,])?;
        input.parse::<Token![->]>()?;
        let output: Type = input.parse()?;
        let body: Block = input.parse()?;
        Ok(DecisionDef { name, inputs: inputs.into_iter().collect(), output, body })
    }
}

impl Parse for FlowDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let in_ty: Type = input.parse()?;
        input.parse::<Token![->]>()?;
        let out_ty: Type = input.parse()?;
        input.parse::<Token![;]>()?;

        let mut entity = None;
        let mut events = Vec::new();
        let mut decisions = Vec::new();
        let mut body = None;

        while !input.is_empty() {
            let kw: Ident = input.parse()?;
            match kw.to_string().as_str() {
                "entity" => {
                    if entity.is_some() {
                        return Err(syn::Error::new(kw.span(), "only one `entity` block per flow"));
                    }
                    entity = Some(input.parse()?);
                }
                "events" => {
                    let content;
                    braced!(content in input);
                    while !content.is_empty() {
                        events.push(content.parse()?);
                    }
                }
                "decision" => decisions.push(input.parse()?),
                "body" => {
                    let paren;
                    parenthesized!(paren in input);
                    let param: Ident = paren.parse()?;
                    let block: Block = input.parse()?;
                    body = Some((param, block));
                }
                other => {
                    return Err(syn::Error::new(kw.span(), format!("unknown section `{other}`; expected entity, events, decision or body")));
                }
            }
        }

        let entity = entity.ok_or_else(|| syn::Error::new(name.span(), "flow needs an `entity` block"))?;
        let (body_param, body) = body.ok_or_else(|| syn::Error::new(name.span(), "flow needs a `body(..) { .. }` block"))?;
        Ok(FlowDef { name, input: in_ty, output: out_ty, entity, events, decisions, body_param, body })
    }
}
