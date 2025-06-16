use core::time::Duration;
use std::{path::PathBuf, process::Stdio, thread, time::Instant};

use clap::{Parser, Subcommand};
use cli_helper::{Action, ActionResult, Module, State, print_fail, print_success};
use color_eyre::eyre::eyre;
use postgres::{Client, NoTls};

use crate::operation_group::OperationGroup;

#[derive(Debug, Parser)]
#[command(name = "sql-helper")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging.
    #[arg(long, action)]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(arg_required_else_help = true)]
    /// Tests the SQL to ensure it is valid.
    Test {
        /// The path to an SQL file or directory containing SQL files.
        source: PathBuf,
    },

    #[command(arg_required_else_help = true)]
    /// Generates bindings to the SQL operations.
    GenerateBindings {
        /// The path to an SQL file or directory containing SQL files.
        source: PathBuf,

        /// The output file to write the bindings to.
        #[arg(short, long)]
        target: Option<PathBuf>,
    },
}

pub fn run_tests(modules: Vec<Module<OperationGroup>>) -> color_eyre::Result<bool> {
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
            "sql-helper-test-db",
            "--detach",
            "postgres:17",
        ])
        .stdout(Stdio::null())
        .spawn()
        .bind_error(&mut action)?
        .wait()
        .bind_error(&mut action)?;

    if !status.success() {
        action.set_state(State::Error);
        return Err(eyre!("Spawning test database failed with status: {status}"));
    }
    action.set_state(State::Success);

    // Connect to DB
    let mut action = Action::new("Connecting", "Connected", "to test database", 0);
    let mut client: Option<Client> = None;
    let start = Instant::now();
    while client.is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            action.set_state(State::Error);
            return Err(eyre!("Could not connect to database"));
        }

        if let Ok(db_client) = Client::connect("postgres://postgres:password@localhost:5432", NoTls)
        {
            client = Some(db_client);
        }

        thread::sleep(Duration::from_millis(100));
    }
    action.set_state(State::Success);

    let mut running_action = Action::new("Testing", "Tested", "SQL", 0);
    running_action.dont_overwrite();

    // Run tests
    let mut client = client.unwrap();
    let mut all_valid = true;

    for module in modules {
        let mut action = Action::new(
            "Testing",
            "Tested",
            format!("{}", module.source.to_string_lossy()),
            1,
        );
        for operation in module.contents.0 {
            all_valid &= operation.is_valid(&mut client, &mut action);
        }
        action.set_state(State::Success);
    }
    running_action.set_state(State::Success);

    if !all_valid {
        println!();
        print_fail("one or more tests failed", 0);

        Ok(false)
    } else {
        println!();
        print_success("all tests passed", 0);
        Ok(true)
    }
}
