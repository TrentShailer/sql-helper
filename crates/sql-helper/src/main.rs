//! # SQL Helper
//! Split helper CLI for working with SQL.

mod cli;
mod operation;
mod operation_group;

use core::{error::Error, fmt, time::Duration};
use std::{
    fs,
    io::{self, Write},
    process::{ExitStatus, Stdio},
    thread,
    time::Instant,
};

use clap::Parser;
use postgres::{Client, NoTls};
use ts_cli_helper::{Action, ActionResult, FileParser, State, print_success};
use ts_rust_helper::error::ReportResult;

use crate::{cli::Cli, operation_group::OperationGroup};

fn main() -> ReportResult<'static, ()> {
    let cli = Cli::parse();

    match cli.command {
        cli::Commands::GenerateBindings { source, target } => {
            let mut database = Database::new("sql-helper-test-database")?;

            let action = Action::new("Parsing", "Parsed", source.to_string_lossy(), 0);
            let file_parser: FileParser<Client, OperationGroup> =
                FileParser::parse(&source, &mut database.client).bind_result(action)?;

            let target_string = match target.as_ref() {
                Some(target) => format!("{target:?}"),
                None => "stdout".to_string(),
            };
            let mut action = Action::new(
                "Writing",
                "Wrote",
                format!("SQL bindings to {target_string}"),
                0,
            );
            if target.is_none() {
                action.dont_overwrite();
            }

            file_parser
                .write(
                    target.as_deref(),
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION"),
                )
                .bind_result(action)?;
        }
        cli::Commands::StartDatabase { migrations } => {
            let mut database = Database::new("sql-helper-dev-database")?;

            if let Some(migrations_path) = migrations {
                let action = Action::new("Reading", "Read", "migrations", 0);
                let sql = fs::read_to_string(migrations_path).bind_result(action)?;

                let action = Action::new("Running", "Ran", "migrations", 0);
                database.client.batch_execute(&sql).bind_result(action)?;
            }

            print_success(
                "Database available at `postgres://postgres:password@localhost:5432`",
                0,
            );

            let mut stdout = io::stdout().lock();
            let _ = stdout.write(b"\nPress enter to kill database")?;
            stdout.flush()?;
            drop(stdout);

            let mut buffer = String::new();
            let _ = io::stdin().read_line(&mut buffer);
        }
    }

    Ok(())
}

struct Database {
    pub client: Client,
    pub name: &'static str,
}

impl Database {
    pub fn new(name: &'static str) -> Result<Self, CreateDatabaseError> {
        // Create database
        let mut action = Action::new("Spawning", "Spawned", "test database", 0);
        let status = std::process::Command::new("docker")
            .args([
                "run",
                "--env",
                "POSTGRES_PASSWORD=password",
                "--publish",
                "5432:5432",
                "--name",
                name,
                "--detach",
                "postgres:17",
            ])
            .stdout(Stdio::null())
            .spawn()
            .bind_error(&mut action)
            .map_err(|source| CreateDatabaseError {
                kind: CreateDatabaseErrorKind::SpawnChild { source },
            })?
            .wait()
            .bind_error(&mut action)
            .map_err(|source| CreateDatabaseError {
                kind: CreateDatabaseErrorKind::SpawnChild { source },
            })?;

        if !status.success() {
            action.set_state(State::Error);
            return Err(CreateDatabaseError {
                kind: CreateDatabaseErrorKind::ChildStatus { status },
            });
        }
        action.set_state(State::Success);

        // Connect to DB
        let mut action = Action::new("Connecting", "Connected", "to test database", 0);
        let mut client: Option<Client> = None;
        let start = Instant::now();
        while client.is_none() {
            if start.elapsed() > Duration::from_secs(10) {
                action.set_state(State::Error);
                return Err(CreateDatabaseError {
                    kind: CreateDatabaseErrorKind::Timeout,
                });
            }

            if let Ok(db_client) =
                Client::connect("postgres://postgres:password@localhost:5432", NoTls)
            {
                client = Some(db_client);
            }

            thread::sleep(Duration::from_millis(100));
        }
        action.set_state(State::Success);

        Ok(Self {
            client: client.unwrap(),
            name,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub struct CreateDatabaseError {
    pub kind: CreateDatabaseErrorKind,
}
impl fmt::Display for CreateDatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to spawn database")
    }
}
impl Error for CreateDatabaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.kind)
    }
}

#[derive(Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum CreateDatabaseErrorKind {
    #[non_exhaustive]
    SpawnChild { source: io::Error },

    #[non_exhaustive]
    ChildStatus { status: ExitStatus },

    #[non_exhaustive]
    Timeout,
}
impl fmt::Display for CreateDatabaseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::SpawnChild { .. } => {
                write!(f, "could not spawn child process")
            }
            Self::ChildStatus { status } => {
                write!(f, "docker process reported exit status {status}")
            }
            Self::Timeout => write!(f, "timedout while connecting"),
        }
    }
}
impl Error for CreateDatabaseErrorKind {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self {
            Self::SpawnChild { source } => Some(source),
            _ => None,
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        let mut child = std::process::Command::new("docker")
            .args(["rm", "--force", self.name])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let status = child.wait().unwrap();
        if !status.success() {
            panic!("Killing dev database failed with status: {status}")
        }
    }
}
