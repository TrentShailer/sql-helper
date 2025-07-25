use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

pub fn create_test(struct_name: &Ident) -> TokenStream {
    let test_name = format_ident!("test_{struct_name}");

    quote! {
        #[cfg(test)]
        #[allow(non_snake_case)]
        #[test]
        fn #test_name() {
            use ts_sql_helper_lib::test::get_test_database;

            let (mut client, _container) = get_test_database();
            let statement = client.prepare(#struct_name::QUERY);
            assert!(statement.is_ok(), "invalid query `{}`: {}", #struct_name::QUERY, statement.unwrap_err());
            let statement = statement.unwrap();

            let mut data: Vec<Box<dyn ts_sql_helper_lib::postgres_types::ToSql + Sync>> = Vec::new();
            let params = statement.params();
            for param in params.iter() {
                match ts_sql_helper_lib::test::data_for_type(param) {
                    Some(param_data) => data.push(param_data),
                    None => panic!("unsupported parameter type `{}`", param.name()),
                }
            }

            let borrowed_data: Vec<&(dyn ts_sql_helper_lib::postgres_types::ToSql + Sync)> =
                data.iter().map(|data| data.as_ref()).collect();

            let result = client.execute(&statement, borrowed_data.as_slice());
            if let Err(error) = result {
                use ts_sql_helper_lib::postgres::error::SqlState;

                assert!(
                    matches!(
                        error.code(),
                        Some(&SqlState::FOREIGN_KEY_VIOLATION) | Some(&SqlState::CHECK_VIOLATION)
                    ),
                    "invalid query `{}`: {error}",
                    #struct_name::QUERY
                );
            }
        }
    }
}
