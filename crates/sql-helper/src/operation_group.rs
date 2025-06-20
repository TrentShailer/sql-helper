use core::{cell::LazyCell, error::Error, fmt};

use cli_helper::{ParseFrom, to_valid_ident};
use postgres::Client;
use quote::{ToTokens, quote};
use regex::Regex;

use crate::operation::Operation;

#[derive(Debug, Clone)]
pub struct OperationGroup(pub Vec<Operation>);

impl ParseFrom<String, Client> for OperationGroup {
    type Error = ParseOperationGroupError;

    fn parse(source: String, state: &mut Client) -> Result<Self, Self::Error> {
        let header_regex: LazyCell<Regex> =
            LazyCell::new(|| Regex::new(r"(?m)^--- (?<name>.+)$").unwrap());

        let headers: Vec<_> = header_regex.captures_iter(&source).collect();
        let bodies: Vec<_> = header_regex.split(&source).collect();
        let bodies = &bodies[1..];

        if headers.is_empty() {
            return Err(Self::Error {
                operation: None,
                kind: ParseOperationGroupErrorKind::NoHeaders,
            });
        }

        if headers.len() != bodies.len() {
            return Err(Self::Error {
                operation: None,
                kind: ParseOperationGroupErrorKind::HeaderBodyMismatch {
                    header_count: headers.len(),
                    body_count: bodies.len(),
                },
            });
        }

        let mut operations = Vec::new();
        for (index, header) in headers.iter().enumerate() {
            let name = to_valid_ident(header.name("name").unwrap().as_str());
            let sql = bodies.get(index).unwrap().trim().to_string();

            let operation = Operation::new(name.clone(), sql, state)?;

            operations.push(operation);
        }

        Ok(Self(operations))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ParseOperationGroupError {
    pub operation: Option<String>,
    pub kind: ParseOperationGroupErrorKind,
}
impl fmt::Display for ParseOperationGroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operation_name = self.operation.as_deref().unwrap_or("group");
        write!(f, "error parsing operation {operation_name}")
    }
}
impl Error for ParseOperationGroupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ParseOperationGroupErrorKind {
    #[non_exhaustive]
    HeaderBodyMismatch {
        header_count: usize,
        body_count: usize,
    },

    #[non_exhaustive]
    NoHeaders,

    #[non_exhaustive]
    NoStatements,

    #[non_exhaustive]
    InvalidSql {
        statement_index: usize,
        source: postgres::Error,
    },

    #[non_exhaustive]
    MismatchedParams {
        statement_index: usize,
        param_index: usize,
        expected: String,
        real: String,
    },

    #[non_exhaustive]
    UnsupportedParameter {
        statement_index: usize,
        param_index: usize,
        param_type: String,
    },
}
impl fmt::Display for ParseOperationGroupErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::HeaderBodyMismatch {
                header_count,
                body_count,
            } => write!(
                f,
                "number of headers ({header_count}) does not equal number of bodies ({body_count})"
            ),
            Self::NoHeaders => write!(
                f,
                "operation group contained no headers. A header is a comment with three hyphens: `--- operation_name`"
            ),
            Self::NoStatements => {
                write!(f, "operation contained no statements")
            }
            Self::InvalidSql {
                statement_index, ..
            } => {
                write!(
                    f,
                    "operation contained invalid SQL in statement {}",
                    statement_index + 1
                )
            }
            Self::MismatchedParams {
                statement_index,
                param_index,
                expected,
                real,
            } => write!(
                f,
                "operation contained mismatched parameters in statement {} parameter ${}, {} != {}",
                statement_index + 1,
                param_index + 1,
                real,
                expected,
            ),
            Self::UnsupportedParameter {
                statement_index,
                param_index,
                param_type,
            } => write!(
                f,
                "operation contained unsupported parameter in statement {} parameter ${} '{}'",
                statement_index + 1,
                param_index + 1,
                param_type,
            ),
        }
    }
}
impl Error for ParseOperationGroupErrorKind {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self {
            Self::InvalidSql { source, .. } => {
                if let Some(source) = source.as_db_error() {
                    Some(source)
                } else {
                    Some(source)
                }
            }
            _ => None,
        }
    }
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
