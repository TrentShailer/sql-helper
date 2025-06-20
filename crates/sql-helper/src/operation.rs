use convert_case::{Case, Casing};
use postgres::{
    Client,
    error::SqlState,
    types::{ToSql, Type},
};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use rand::{Rng, distr::Alphanumeric, random_bool};
use sql_helper_lib::{SqlDate, SqlDateTime, SqlTime, SqlTimestamp};
use uuid::Uuid;

use crate::operation_group::{ParseOperationGroupError, ParseOperationGroupErrorKind};

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: String,
    pub statements: Vec<String>,
    pub params: Vec<Type>,
}

impl Operation {
    pub fn new(
        name: String,
        sql: String,
        client: &mut Client,
    ) -> Result<Self, ParseOperationGroupError> {
        let statements: Vec<String> = sql
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
        })
    }

    fn data_for_type(param: &Type) -> Option<Box<dyn ToSql + Sync>> {
        match param {
            &Type::BOOL => Some(Box::new(random_bool(0.5))),
            &Type::BYTEA => {
                let mut bytes = vec![0u8; 32];
                rand::rng().fill(bytes.as_mut_slice());
                Some(Box::new(bytes))
            }
            &Type::CHAR => Some(Box::new(rand::random::<i8>())),
            &Type::INT8 => Some(Box::new(rand::random::<i64>())),
            &Type::INT4 => Some(Box::new(rand::random::<i32>())),
            &Type::INT2 => Some(Box::new(rand::random::<i16>())),
            &Type::FLOAT8 => Some(Box::new(rand::random::<f64>())),
            &Type::FLOAT4 => Some(Box::new(rand::random::<f32>())),
            &Type::TEXT | &Type::VARCHAR => {
                let string = rand::rng()
                    .sample_iter(&Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect::<String>();
                Some(Box::new(string))
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
            &Type::TIMESTAMPTZ => Some(Box::new(SqlTimestamp(jiff::Timestamp::now()))),
            &Type::DATE => Some(Box::new(SqlDate(jiff::civil::date(2024, 2, 29)))),
            &Type::TIME => Some(Box::new(SqlTime(jiff::civil::time(21, 30, 5, 123_456_789)))),
            &Type::UUID => Some(Box::new(Uuid::new_v4())),

            _ => None,
        }
    }

    fn parameter_tokens(&self) -> TokenStream {
        let struct_name = format_ident!("{}Params", self.name.to_case(Case::UpperCamel));

        let argument_names: Vec<syn::Ident> = self
            .params
            .iter()
            .enumerate()
            .map(|(index, _)| format_ident!("param_{index}"))
            .collect();

        let fields: Vec<TokenStream> = self
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let param_type: syn::Type = match param {
                    &Type::BOOL => syn::parse_quote!(&'a bool),
                    &Type::BYTEA => syn::parse_quote!(&'a [u8]),
                    &Type::CHAR => syn::parse_quote!(&'a i8),
                    &Type::INT8 => syn::parse_quote!(&'a i64),
                    &Type::INT4 => syn::parse_quote!(&'a i32),
                    &Type::INT2 => syn::parse_quote!(&'a i16),
                    &Type::FLOAT8 => syn::parse_quote!(&'a f64),
                    &Type::FLOAT4 => syn::parse_quote!(&'a f32),
                    &Type::UUID => syn::parse_quote!(&'a uuid::Uuid),
                    &Type::TEXT | &Type::VARCHAR => syn::parse_quote!(&'a str),
                    &Type::TIMESTAMP => syn::parse_quote!(&'a sql_helper_lib::SqlDateTime),
                    &Type::TIMESTAMPTZ => syn::parse_quote!(&'a sql_helper_lib::SqlTimestamp),
                    &Type::DATE => syn::parse_quote!(&'a sql_helper_lib::SqlDate),
                    &Type::TIME => syn::parse_quote!(&'a sql_helper_lib::SqlTime),

                    _ => unreachable!(),
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
            .map(|step| format!("```sql\n{}\n```", step))
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
