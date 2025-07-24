//! Utilities for testing SQL on a live database

use core::marker::PhantomData;

use crate::perform_migrations;

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
