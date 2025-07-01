use core::{cell::LazyCell, error::Error, fmt};

use convert_case::{Case, Casing};
use postgres::{
    Client,
    error::SqlState,
    types::{ToSql, Type},
};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use rand::{Rng, distr::Alphanumeric, random_bool};
use regex::{Captures, Regex};
use ts_sql_helper_lib::{SqlDate, SqlDateTime, SqlTime, SqlTimestamp};
use uuid::Uuid;

use crate::operation_group::{ParseOperationGroupError, ParseOperationGroupErrorKind};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Operator {
    Optional { parameter_index: usize },
}
impl TryFrom<Captures<'_>> for Operator {
    type Error = OperatorError;

    fn try_from(value: Captures<'_>) -> Result<Self, Self::Error> {
        let op_code = value.name("op").unwrap().as_str().trim();
        let params = value.name("params").unwrap().as_str().trim();

        match op_code {
            "opt" => {
                let parameter: usize = params
                    .get(1..)
                    .ok_or_else(|| OperatorError {
                        op_code: op_code.to_string(),
                        params: params.to_string(),
                        kind: OperatorErrorKind::InvalidParams {
                            expected: "a SQL parameter",
                            example: "$3",
                        },
                    })?
                    .parse()
                    .map_err(|_| OperatorError {
                        op_code: op_code.to_string(),
                        params: params.to_string(),
                        kind: OperatorErrorKind::InvalidParams {
                            expected: "a SQL parameter",
                            example: "$3",
                        },
                    })?;
                Ok(Self::Optional {
                    parameter_index: parameter - 1,
                })
            }

            op_code => unimplemented!("operator code `{op_code}`"),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct OperatorError {
    pub op_code: String,
    pub params: String,
    pub kind: OperatorErrorKind,
}
impl fmt::Display for OperatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error parsing operator `{} {}`",
            self.op_code, self.params,
        )
    }
}
impl Error for OperatorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
}
#[derive(Debug)]
#[non_exhaustive]
pub enum OperatorErrorKind {
    #[non_exhaustive]
    InvalidParams {
        expected: &'static str,
        example: &'static str,
    },
}
impl fmt::Display for OperatorErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::InvalidParams {
                expected, example, ..
            } => {
                write!(
                    f,
                    "invalid parameters: expected {expected}, e.g., `{example}`"
                )
            }
        }
    }
}
impl Error for OperatorErrorKind {}

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: String,
    pub statements: Vec<String>,
    pub params: Vec<Type>,
    pub operators: Vec<Operator>,
}

impl Operation {
    pub fn new(
        name: String,
        sql: String,
        client: &mut Client,
    ) -> Result<Self, ParseOperationGroupError> {
        // Regex to extract operators
        let operator_regex: LazyCell<Regex> =
            LazyCell::new(|| Regex::new(r"(?m)^-- (?<op>opt) (?<params>.*)$").unwrap());
        let operators: Vec<_> = operator_regex
            .captures_iter(&sql)
            .map(Operator::try_from)
            .collect::<Result<_, _>>()
            .map_err(|source| ParseOperationGroupError {
                operation: Some(name.clone()),
                kind: ParseOperationGroupErrorKind::OperatorError { source },
            })?;

        // Regex to remove comments
        let comment_regex: LazyCell<Regex> = LazyCell::new(|| Regex::new(r"(?m)--.*").unwrap());

        let statements: Vec<String> = comment_regex
            .replace_all(&sql, "")
            .split_inclusive(';')
            .filter_map(|statement| {
                let sql = statement.trim();
                if sql.is_empty() {
                    None
                } else {
                    Some(sql.to_string())
                }
            })
            .collect();

        if statements.is_empty() {
            return Err(ParseOperationGroupError {
                operation: Some(name.clone()),
                kind: ParseOperationGroupErrorKind::NoStatements,
            });
        }

        let mut operation_params = vec![];

        for (statement_index, statement) in statements.iter().enumerate() {
            // Ensure statement is valid SQL
            let prepared_statement =
                client
                    .prepare(statement)
                    .map_err(|source| ParseOperationGroupError {
                        operation: Some(name.clone()),
                        kind: ParseOperationGroupErrorKind::InvalidSql {
                            statement_index,
                            source,
                        },
                    })?;

            // Check for mismatched params and build data
            let mut data: Vec<Box<dyn ToSql + Sync>> = Vec::new();

            let params = prepared_statement.params();
            operation_params.extend_from_slice(&params[operation_params.len()..]);
            for (param_index, param) in params.iter().enumerate() {
                let expected = &operation_params[param_index];
                if param != expected {
                    return Err(ParseOperationGroupError {
                        operation: Some(name.clone()),
                        kind: ParseOperationGroupErrorKind::MismatchedParams {
                            statement_index,
                            param_index,
                            expected: expected.to_string(),
                            real: param.to_string(),
                        },
                    });
                }

                data.push(
                    Self::data_for_type(param).ok_or_else(|| ParseOperationGroupError {
                        operation: Some(name.clone()),
                        kind: ParseOperationGroupErrorKind::UnsupportedParameter {
                            statement_index,
                            param_index,
                            param_type: param.to_string(),
                        },
                    })?,
                );
            }

            let borrowed_data: Vec<&(dyn ToSql + Sync)> =
                data.iter().map(|data| data.as_ref()).collect();

            if let Err(error) = client.execute(statement, borrowed_data.as_slice()) {
                if let Some(error) = error.as_db_error() {
                    match error.code() {
                        &SqlState::FOREIGN_KEY_VIOLATION | &SqlState::CHECK_VIOLATION => continue,
                        _ => {}
                    }
                }

                return Err(ParseOperationGroupError {
                    operation: Some(name.clone()),
                    kind: ParseOperationGroupErrorKind::InvalidSql {
                        statement_index,
                        source: error,
                    },
                });
            }
        }

        Ok(Self {
            name,
            statements,
            params: operation_params,
            operators,
        })
    }

    fn data_for_type(param: &Type) -> Option<Box<dyn ToSql + Sync>> {
        match param {
            &Type::BOOL => Some(Box::new(random_bool(0.5))),
            &Type::BOOL_ARRAY => {
                let mut data = vec![false; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::BYTEA => {
                let mut bytes = vec![0u8; 32];
                rand::rng().fill(bytes.as_mut_slice());
                Some(Box::new(bytes))
            }
            &Type::BYTEA_ARRAY => {
                let mut bytes = vec![0u8; 32];
                rand::rng().fill(bytes.as_mut_slice());
                let bytes = vec![bytes; 2];
                Some(Box::new(bytes))
            }
            &Type::CHAR => Some(Box::new(rand::random::<i8>())),
            &Type::CHAR_ARRAY => {
                let mut data = vec![0i8; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::INT8 => Some(Box::new(rand::random::<i64>())),
            &Type::INT8_ARRAY => {
                let mut data = vec![0i64; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::INT4 => Some(Box::new(rand::random::<i32>())),
            &Type::INT4_ARRAY => {
                let mut data = vec![0i32; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::INT2 => Some(Box::new(rand::random::<i16>())),
            &Type::INT2_ARRAY => {
                let mut data = vec![0i16; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::FLOAT8 => Some(Box::new(rand::random::<f64>())),
            &Type::FLOAT8_ARRAY => {
                let mut data = vec![0f64; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::FLOAT4 => Some(Box::new(rand::random::<f32>())),
            &Type::FLOAT4_ARRAY => {
                let mut data = vec![0f32; 4];
                rand::rng().fill(data.as_mut_slice());
                Some(Box::new(data))
            }
            &Type::TEXT | &Type::VARCHAR => {
                let string = rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect::<String>();
                Some(Box::new(string))
            }
            &Type::TEXT_ARRAY | &Type::VARCHAR_ARRAY => {
                let data = rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(4)
                    .map(char::from)
                    .collect::<String>();
                Some(Box::new(vec![data; 4]))
            }
            &Type::TIMESTAMP => Some(Box::new(SqlDateTime(jiff::civil::DateTime::constant(
                2024,
                2,
                29,
                21,
                30,
                5,
                123_456_789,
            )))),
            &Type::TIMESTAMP_ARRAY => {
                let data = SqlDateTime(jiff::civil::DateTime::constant(
                    2024,
                    2,
                    29,
                    21,
                    30,
                    5,
                    123_456_789,
                ));
                Some(Box::new(vec![data; 4]))
            }
            &Type::TIMESTAMPTZ => Some(Box::new(SqlTimestamp(jiff::Timestamp::now()))),
            &Type::TIMESTAMPTZ_ARRAY => {
                let data = SqlTimestamp(jiff::Timestamp::now());
                Some(Box::new(vec![data; 4]))
            }
            &Type::DATE => Some(Box::new(SqlDate(jiff::civil::date(2024, 2, 29)))),
            &Type::DATE_ARRAY => {
                let data = SqlDate(jiff::civil::date(2024, 2, 29));
                Some(Box::new(vec![data; 4]))
            }
            &Type::TIME => Some(Box::new(SqlTime(jiff::civil::time(21, 30, 5, 123_456_789)))),
            &Type::TIME_ARRAY => {
                let data = SqlTime(jiff::civil::time(21, 30, 5, 123_456_789));
                Some(Box::new(vec![data; 4]))
            }
            &Type::UUID => Some(Box::new(Uuid::new_v4())),
            &Type::UUID_ARRAY => {
                let data = Uuid::new_v4();
                Some(Box::new(vec![data; 4]))
            }

            _ => None,
        }
    }

    fn parameter_tokens(&self) -> TokenStream {
        let struct_name = format_ident!("{}Params", self.name.to_case(Case::UpperCamel));

        let argument_names: Vec<syn::Ident> = self
            .params
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("p{}", index + 1))
            .collect();

        let fields: Vec<TokenStream> = self
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let param_type: syn::Type = match param {
                    &Type::BOOL => syn::parse_quote!(&'a bool),
                    &Type::BOOL_ARRAY => syn::parse_quote!(&'a bool),
                    &Type::BYTEA => syn::parse_quote!(&'a [Vec<u8>]),
                    &Type::BYTEA_ARRAY => syn::parse_quote!(&'a [u8]),
                    &Type::CHAR => syn::parse_quote!(&'a i8),
                    &Type::CHAR_ARRAY => syn::parse_quote!(&'a [i8]),
                    &Type::INT8 => syn::parse_quote!(&'a i64),
                    &Type::INT8_ARRAY => syn::parse_quote!(&'a [i64]),
                    &Type::INT4 => syn::parse_quote!(&'a i32),
                    &Type::INT4_ARRAY => syn::parse_quote!(&'a [i32]),
                    &Type::INT2 => syn::parse_quote!(&'a i16),
                    &Type::INT2_ARRAY => syn::parse_quote!(&'a [i16]),
                    &Type::FLOAT8 => syn::parse_quote!(&'a f64),
                    &Type::FLOAT8_ARRAY => syn::parse_quote!(&'a [f64]),
                    &Type::FLOAT4 => syn::parse_quote!(&'a f32),
                    &Type::FLOAT4_ARRAY => syn::parse_quote!(&'a [f32]),
                    &Type::UUID => syn::parse_quote!(&'a uuid::Uuid),
                    &Type::UUID_ARRAY => syn::parse_quote!(&'a [uuid::Uuid]),
                    &Type::TEXT | &Type::VARCHAR => {
                        syn::parse_quote!(&'a str)
                    }
                    &Type::VARCHAR_ARRAY | &Type::TEXT_ARRAY => syn::parse_quote!(&'a [String]),
                    &Type::TIMESTAMP => {
                        syn::parse_quote!(&'a sql_helper_lib::SqlDateTime)
                    }
                    &Type::TIMESTAMP_ARRAY => syn::parse_quote!(&'a [sql_helper_lib::SqlDateTime]),
                    &Type::TIMESTAMPTZ => {
                        syn::parse_quote!(&'a sql_helper_lib::SqlTimestamp)
                    }
                    &Type::TIMESTAMPTZ_ARRAY => {
                        syn::parse_quote!(&'a [sql_helper_lib::SqlTimestamp])
                    }
                    &Type::DATE => {
                        syn::parse_quote!(&'a sql_helper_lib::SqlDate)
                    }
                    &Type::DATE_ARRAY => syn::parse_quote!(&'a [sql_helper_lib::SqlDate]),
                    &Type::TIME => {
                        syn::parse_quote!(&'a sql_helper_lib::SqlTime)
                    }
                    &Type::TIME_ARRAY => syn::parse_quote!(&'a [sql_helper_lib::SqlTime]),

                    _ => unreachable!(),
                };

                let is_optional = self.operators.iter().any(|operator| {
                    #[expect(irrefutable_let_patterns)]
                    if let Operator::Optional { parameter_index } = operator {
                        return parameter_index == &index;
                    }
                    false
                });

                let param_type = if is_optional {
                    syn::parse_quote!(Option<#param_type>)
                } else {
                    param_type
                };

                let name = &argument_names[index];

                quote! {
                    pub #name: #param_type
                }
            })
            .collect();

        let self_params: Vec<_> = argument_names
            .iter()
            .map(|name| quote! {self.#name})
            .collect();

        let items = self.params.len();

        quote! {
            pub struct #struct_name<'a> {
                #( #fields, )*
                pub phantom_data: core::marker::PhantomData<&'a ()>
            }
            impl<'a> #struct_name<'a> {
                pub fn params(&'a self) -> [&'a (dyn postgres::types::ToSql + Sync); #items] {
                    [
                        #( &#self_params ),*
                    ]
                }
            }
        }
    }
}

impl ToTokens for Operation {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = format_ident!("{}", self.name);

        let steps = &self.statements;
        let count: usize = steps.len();

        let doc_string = self
            .statements
            .iter()
            .map(|step| format!("```sql\n{step}\n```"))
            .collect::<Vec<_>>()
            .join("\n\n");

        let doc_string = format!("# SQL\n{doc_string}");

        let parameter_function = self.parameter_tokens();

        let new_tokens = quote! {
            #[doc = #doc_string]
            pub fn #name() -> [&'static str; #count] {
                [#( #steps ),*]
            }

            #parameter_function
        };

        tokens.extend(new_tokens);
    }
}
