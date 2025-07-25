//! Utilities for testing SQL on a live database

use postgres_types::{ToSql, Type};
use rand::{Rng, distr::Alphanumeric, random_bool};
use testcontainers::{Container, runners::SyncRunner};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

use crate::{SqlDate, SqlDateTime, SqlTime, SqlTimestamp, perform_migrations};

/// Creates a test database container for the test
pub fn get_test_database() -> (postgres::Client, Container<Postgres>) {
    let container = Postgres::default().start().unwrap();
    let host_ip = container.get_host().unwrap();
    let host_port = container.get_host_port_ipv4(5432).unwrap();

    let connection_string = format!("postgres://postgres:postgres@{host_ip}:{host_port}/postgres");
    let mut client = postgres::Client::connect(&connection_string, postgres::NoTls).unwrap();
    perform_migrations(&mut client).unwrap();

    (client, container)
}

/// Generate some random data for a given type.
pub fn data_for_type(param: &Type) -> Option<Box<dyn ToSql + Sync>> {
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
            let data = (0..4)
                .map(|_| {
                    rand::rng()
                        .sample_iter(&Alphanumeric)
                        .take(4)
                        .map(char::from)
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            Some(Box::new(data))
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
            let data = (0..4).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
            Some(Box::new(data))
        }

        _ => None,
    }
}
