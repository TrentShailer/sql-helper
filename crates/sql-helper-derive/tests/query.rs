#![allow(missing_docs)]

use ts_sql_helper_derive::query;

#[allow(unused)]
fn test() {
    query! {name: Basic, query: ""};
    Basic::params().as_array().as_slice();

    query! {
        name: Advanced,
        optional_params: [1],
        query: "
            UPDATE
                public_keys
            SET
                last_used = $1::VARCHAR,
                signature_counter = $2
            WHERE
                raw_id = $3::BYTEA"
    };
    Advanced::params(Some(""), &"", &[0]).as_array().as_slice();
}
