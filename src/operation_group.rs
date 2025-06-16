use core::cell::LazyCell;

use cli_helper::to_valid_ident;
use quote::{ToTokens, quote};
use regex::Regex;
use thiserror::Error;

use crate::operation::Operation;

#[derive(Debug, Clone)]
pub struct OperationGroup(pub Vec<Operation>);

impl TryFrom<String> for OperationGroup {
    type Error = OperationGroupError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let header_regex: LazyCell<Regex> =
            LazyCell::new(|| Regex::new(r"(?m)^--- (?<name>.+)$").unwrap());

        let headers: Vec<_> = header_regex.captures_iter(&value).collect();
        let bodies: Vec<_> = header_regex.split(&value).collect();
        let bodies = &bodies[1..];

        if headers.is_empty() {
            return Err(Self::Error::NoHeaders);
        }

        if headers.len() != bodies.len() {
            return Err(Self::Error::HeaderBodyMismatch(headers.len(), bodies.len()));
        }

        let mut operations = Vec::new();
        for (index, header) in headers.iter().enumerate() {
            let name = to_valid_ident(header.name("name").unwrap().as_str());
            let sql = bodies.get(index).unwrap().trim().to_string();

            let operation = Operation::new(name, sql);
            if operation.steps.is_empty() {
                return Err(Self::Error::NoSteps(operation.name));
            }

            operations.push(operation);
        }

        Ok(Self(operations))
    }
}

#[derive(Error, Debug)]
pub enum OperationGroupError {
    #[error("Number of headers ({0}) does not equal number of bodies ({1}).")]
    HeaderBodyMismatch(usize, usize),

    #[error(
        "Contents contain no headers. A header is a line with starting with three dashes and a title: `--- title_here`"
    )]
    NoHeaders,

    #[error("Operation '{0}' contains no steps.")]
    NoSteps(String),
}

impl ToTokens for OperationGroup {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let operations = &self.0;
        let new_tokens = quote! {
            #( #operations )*
        };
        tokens.extend(new_tokens);
    }
}
