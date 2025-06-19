use std::time::SystemTime;

use cli_helper::{Action, print_error, print_fail, print_warning};
use postgres::{Client, error::SqlState, types::ToSql};
use quote::{ToTokens, format_ident, quote};
use rand::{Rng, distr::Alphanumeric, random_bool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Operation {
    pub name: String,
    pub steps: Vec<String>,
}

impl Operation {
    pub fn new(name: String, sql: String) -> Self {
        let steps: Vec<String> = sql
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

        Self { name, steps }
    }

    pub fn is_valid(&self, client: &mut Client, action: &mut Action) -> bool {
        let mut is_valid = true;

        'step: for (index, step) in self.steps.iter().enumerate() {
            let statement = match client.prepare(step) {
                Ok(statement) => statement,
                Err(error) => {
                    is_valid = false;

                    print_fail(
                        &format!(
                            "Operation '{}' failed test on step {}",
                            self.name,
                            index + 1,
                        ),
                        action.indent + 1,
                    );
                    print_error(&error.to_string(), action.indent + 2);

                    action.dont_overwrite();

                    continue;
                }
            };

            if statement.params().is_empty() {
                if let Err(error) = client.execute(&statement, &[]) {
                    is_valid = false;

                    print_fail(
                        &format!(
                            "Operation '{}' failed test on step {}",
                            self.name,
                            index + 1,
                        ),
                        action.indent + 1,
                    );
                    print_error(&error.to_string(), action.indent + 2);

                    action.dont_overwrite();
                }
            } else {
                let mut params = Vec::<Box<(dyn ToSql + Sync)>>::new();

                for param in statement.params().iter().map(|param| param.name()) {
                    match param {
                        "bool" => params.push(Box::new(random_bool(0.5))),
                        "bytea" => {
                            let mut bytes = vec![0u8; 32];
                            rand::rng().fill(bytes.as_mut_slice());
                            params.push(Box::new(bytes));
                        }
                        "char" => params.push(Box::new(rand::random::<i8>())),
                        "int8" => params.push(Box::new(rand::random::<i64>())),
                        "int4" => params.push(Box::new(rand::random::<i32>())),
                        "int2" => params.push(Box::new(rand::random::<i16>())),
                        "float8" => params.push(Box::new(rand::random::<f64>())),
                        "float4" => params.push(Box::new(rand::random::<f32>())),
                        "text" | "varchar" => {
                            let string = rand::rng()
                                .sample_iter(&Alphanumeric)
                                .take(32)
                                .map(char::from)
                                .collect::<String>();
                            params.push(Box::new(string));
                        }
                        "timestamp" | "timestamptz" => params.push(Box::new(SystemTime::now())),
                        "uuid" => params.push(Box::new(Uuid::new_v4())),

                        param_type => {
                            print_warning(
                                &format!(
                                    "could not test execution of operation '{}' step {}, generating data for '{}' is unsupported",
                                    self.name,
                                    index + 1,
                                    param_type
                                ),
                                action.indent + 1,
                            );
                            action.dont_overwrite();
                            continue 'step;
                        }
                    }
                }
                let borrowed_params: Vec<_> = params.iter().map(|param| param.as_ref()).collect();
                if let Err(error) = client.execute(&statement, borrowed_params.as_slice()) {
                    if let Some(error) = error.as_db_error() {
                        match error.code() {
                            &SqlState::FOREIGN_KEY_VIOLATION | &SqlState::CHECK_VIOLATION => {
                                continue;
                            }
                            _ => {}
                        }
                    }

                    is_valid = false;

                    print_fail(
                        &format!(
                            "Operation '{}' failed test on step {}",
                            self.name,
                            index + 1,
                        ),
                        action.indent + 1,
                    );
                    print_error(&error.to_string(), action.indent + 2);

                    action.dont_overwrite();
                }
            }
        }

        is_valid
    }
}

impl ToTokens for Operation {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = format_ident!("{}", self.name);

        let steps = &self.steps;
        let count: usize = steps.len();

        let doc_string = self
            .steps
            .iter()
            .map(|step| format!("```sql\n{}\n```", step))
            .collect::<Vec<_>>()
            .join("\n\n");

        let doc_string = format!("# SQL\n{doc_string}");

        let new_tokens = quote! {
                    #[doc = #doc_string]
            pub fn #name() -> [&'static str; #count] {
                [#( #steps ),*]
            }
        };

        tokens.extend(new_tokens);
    }
}
