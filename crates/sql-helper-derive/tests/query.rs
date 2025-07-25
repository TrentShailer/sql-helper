#![allow(missing_docs)]

use ts_sql_helper_derive::query;

query! {name: Test, query: ""}
query! {name: Basic, query: ""}
query! {
    name: Advanced,
    optional_params: [1],
    query: r#"
        INSERT INTO challenges(challenge, origin)
        VALUES ($1::BYTEA, $2::VARCHAR);"#
}
