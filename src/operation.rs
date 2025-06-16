use cli_helper::{Action, print_error, print_fail};
use postgres::Client;
use quote::{ToTokens, format_ident, quote};

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

        for (index, step) in self.steps.iter().enumerate() {
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
