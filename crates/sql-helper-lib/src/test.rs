//! Utilities for testing SQL on a live database

use core::marker::PhantomData;

use postgres_types::{ToSql, Type};
use rand::{Rng, distr::Alphanumeric, random_bool};
use uuid::Uuid;

use crate::{SqlDate, SqlDateTime, SqlTime, SqlTimestamp, perform_migrations};

static COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// Struct that will teardown the database once all tests have finished
pub struct TestTeardown {
    phantom_data: PhantomData<()>,
}
impl TestTeardown {
    #[allow(clippy::new_without_default)]
    /// Create a new teardown entry
    pub fn new() -> Self {
        COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        Self {
            phantom_data: PhantomData,
        }
    }
}
impl Drop for TestTeardown {
    fn drop(&mut self) {
        use std::process::Stdio;

        if COUNTER.fetch_sub(1, core::sync::atomic::Ordering::SeqCst) != 1 {
            return;
        }

        let mut child = std::process::Command::new("docker")
            .args(["rm", "--force", "test-database"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let status = child.wait().unwrap();
        assert!(status.success());
    }
}

/// Function to create a client for tests on first access then subsequent tests reuse the client.
pub fn get_test_database_connection() -> (
    std::sync::Arc<std::sync::Mutex<postgres::Client>>,
    TestTeardown,
) {
    use std::io;
    use std::sync::{Arc, Mutex, OnceLock};

    use postgres::NoTls;

    static CLIENT: OnceLock<Arc<Mutex<postgres::Client>>> = OnceLock::new();

    let client = CLIENT
        .get_or_init(|| {
            use std::process::Stdio;

            let mut child = std::process::Command::new("docker")
                .args([
                    "run",
                    "--env",
                    "POSTGRES_PASSWORD=password",
                    "--publish",
                    "5555:5555",
                    "--name",
                    "test-database",
                    "--detach",
                    "postgres:17",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();

            let status = child.wait().unwrap();

            let stdout = io::read_to_string(child.stdout.take().unwrap()).unwrap();
            let stderr = io::read_to_string(child.stderr.take().unwrap()).unwrap();

            assert!(status.success(), "stdout: {stdout}\n\nstderr: {stderr}");

            let mut client =
                postgres::Client::connect("postgres://postgres:password@localhost:5432", NoTls)
                    .unwrap();
            perform_migrations(&mut client).unwrap();
            Arc::new(Mutex::new(client))
        })
        .clone();

    (client, TestTeardown::new())
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
