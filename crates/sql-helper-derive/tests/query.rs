#![allow(missing_docs)]

use ts_sql_helper_derive::query;
use ts_sql_helper_lib::{FromRow, SqlTimestamp};

query! {
    name: CreateChallenge,
    query: r#"
        INSERT INTO challenges (challenge, origin)  
        VALUES ($1::BYTEA, $2::VARCHAR)"#
}

query! {
    name: GetChallenge,
    row: {
        challenge: Vec<u8>,
        origin: String,
        issued: SqlTimestamp,
        expires: SqlTimestamp
    },
    query: r#"
        SELECT
            challenge,
            origin,
            issued,
            expires
        FROM
            challenges
        WHERE
            challenge = $1::BYTEA;"#
}

#[test]
fn real_test() {
    let (mut client, _container) = ts_sql_helper_lib::test::get_test_database();

    let rows_modified = client
        .execute(
            CreateChallenge::QUERY,
            CreateChallenge::params(&[0, 1, 2, 3, 4], "some-origin")
                .as_array()
                .as_slice(),
        )
        .unwrap();
    assert_eq!(rows_modified, 1);

    let rows = client
        .query(
            GetChallenge::QUERY,
            GetChallenge::params(&[0, 1, 2, 3, 4]).as_array().as_slice(),
        )
        .unwrap();
    let rows: Vec<_> = rows
        .into_iter()
        .map(|row| GetChallengeRow::from_row(&row).unwrap())
        .collect();
    assert!(!rows.is_empty());

    let row = &rows[0];
    assert_eq!(row.challenge, vec![0, 1, 2, 3, 4]);
    assert_eq!(row.origin, "some-origin");
}
