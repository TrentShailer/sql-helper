#![allow(missing_docs)]

use ts_sql_helper_derive::query;

query! {name: Test, query: ""}
query! {name: Basic, query: ""}
query! {
    name: Advanced,
    optional_params: [1],
    query: r#"
            UPDATE
                public_keys
            SET
                last_used = $1::TIMESTAMPTZ,
                signature_counter = $2
            WHERE
                raw_id = $3::BYTEA"#
}
