use std::convert::TryFrom;

use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Attribute, Error, FnArg, Lit, LitStr, Meta};
use syn::{NestedMeta, Pat, PatType, Path, Result, Signature, Token, Type};

pub struct AttributeArgs(pub Vec<String>);

impl Parse for AttributeArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut v = Vec::new();

        loop {
            if input.is_empty() {
                break;
            }

            v.push(input.parse::<LitStr>()?.value());

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(Self(v))
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Ident(Ident),
    Lit(Lit),
}

impl ToTokens for Value {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Value::Ident(ident) => ident.to_tokens(tokens),
            Value::Lit(lit) => lit.to_tokens(tokens),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Attr {
    pub path: Path,
    pub values: Vec<Value>,
}

impl Attr {
    pub fn new(path: Path, values: Vec<Value>) -> Self {
        Self {
            path,
            values,
        }
    }
}

impl ToTokens for Attr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Attr {
            path,
            values,
        } = self;

        tokens.extend(if values.is_empty() {
            quote!(#[#path])
        } else {
            quote!(#[#path(#(#values)*,)])
        });
    }
}

impl TryFrom<&Attribute> for Attr {
    type Error = Error;

    fn try_from(attr: &Attribute) -> Result<Self> {
        parse_attribute(attr)
    }
}

pub fn parse_attribute(attr: &Attribute) -> Result<Attr> {
    let meta = attr.parse_meta()?;

    match meta {
        Meta::Path(p) => Ok(Attr::new(p, Vec::new())),
        Meta::List(l) => {
            let path = l.path;
            let values = l
                .nested
                .into_iter()
                .map(|m| match m {
                    NestedMeta::Lit(lit) => Ok(Value::Lit(lit)),
                    NestedMeta::Meta(m) => match m {
                        Meta::Path(p) => Ok(Value::Ident(p.get_ident().unwrap().clone())),
                        _ => Err(Error::new(
                            m.span(),
                            "nested lists or name values are not supported",
                        )),
                    },
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(Attr::new(path, values))
        },
        Meta::NameValue(nv) => Ok(Attr::new(nv.path, vec![Value::Lit(nv.lit)])),
    }
}

pub fn parse_identifiers(attr: &Attr) -> Result<Vec<Ident>> {
    attr.values
        .iter()
        .map(|v| match v {
            Value::Ident(ident) => Ok(ident.clone()),
            Value::Lit(lit) => Err(Error::new(lit.span(), "literals are forbidden")),
        })
        .collect::<Result<Vec<_>>>()
}

pub fn parse_value<T>(attr: &Attr, f: impl FnOnce(&Value) -> Result<T>) -> Result<T> {
    if attr.values.is_empty() {
        return Err(Error::new(attr.span(), "attribute input must not be empty"));
    }

    if attr.values.len() > 1 {
        return Err(Error::new(
            attr.span(),
            "attribute input must not exceed more than one argument",
        ));
    }

    f(&attr.values[0])
}

pub fn parse_identifier(attr: &Attr) -> Result<Ident> {
    parse_value(attr, |value| {
        Ok(match value {
            Value::Ident(ident) => ident.clone(),
            _ => return Err(Error::new(value.span(), "argument must be an identifier")),
        })
    })
}

pub fn parse_string(attr: &Attr) -> Result<String> {
    parse_value(attr, |value| {
        Ok(match value {
            Value::Lit(Lit::Str(s)) => s.value(),
            _ => return Err(Error::new(value.span(), "argument must be a string")),
        })
    })
}

pub fn parse_bool(attr: &Attr) -> Result<bool> {
    parse_value(attr, |value| {
        Ok(match value {
            Value::Lit(Lit::Bool(b)) => b.value,
            _ => return Err(Error::new(value.span(), "argument must be a boolean")),
        })
    })
}

pub fn get_first_two_parameters(sig: &Signature) -> Result<(&FnArg, &FnArg)> {
    let mut parameters = sig.inputs.iter();
    match (parameters.next(), parameters.next()) {
        (Some(first), Some(second)) => Ok((first, second)),
        _ => Err(Error::new(
            sig.inputs.span(),
            "the function must have a context and a message parameter",
        )),
    }
}

pub fn get_pat_type(arg: &FnArg) -> Result<&PatType> {
    match arg {
        FnArg::Typed(t) => Ok(t),
        _ => Err(Error::new(arg.span(), "`self` cannot be used as the context type")),
    }
}

pub fn get_ident(p: &Pat) -> Result<Ident> {
    match p {
        Pat::Ident(pi) => Ok(pi.ident.clone()),
        _ => Err(Error::new(p.span(), "parameter must have an identifier")),
    }
}

pub fn get_path(t: &Type) -> Result<&Path> {
    match t {
        Type::Path(p) => Ok(&p.path),
        Type::Reference(r) => get_path(&r.elem),
        _ => Err(Error::new(t.span(), "parameter must be a path to a context type")),
    }
}
