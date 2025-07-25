//! # SQL Helper
//! Split helper CLI for working with SQL.

use std::{
    io::{self, Write},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use testcontainers::runners::SyncRunner;
use testcontainers_modules::postgres::Postgres;
use ts_cli_helper::{Action, ActionResult, print_success};
use ts_rust_helper::error::ReportProgramExit;
use ts_sql_helper_lib::perform_migrations;

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
    /// Creates a database for testing.
    StartDatabase {
        /// Path to a directory containing migrations to set up the database.
        #[arg(short, long)]
        migrations: Option<PathBuf>,
    },
}

fn main() -> ReportProgramExit {
    let cli = Cli::parse();

    match cli.command {
        Commands::StartDatabase { migrations } => {
            let mut action = Action::new("Starting", "Started", "database container", 0);

            let container = Postgres::default().start().bind_error(&mut action)?;
            let host_ip = container.get_host().bind_error(&mut action)?;
            let host_port = container.get_host_port_ipv4(5432).bind_result(action)?;

            let connection_string =
                format!("postgres://postgres:postgres@{host_ip}:{host_port}/postgres");

            // Perform migrations
            {
                let action = Action::new("Connecting", "Connected", "to database", 0);
                let mut client = postgres::Client::connect(&connection_string, postgres::NoTls)
                    .bind_result(action)?;

                let action = Action::new("Running", "Ran", "migrations", 0);
                perform_migrations(&mut client, migrations).bind_result(action)?;
            }

            print_success(format!("Database available at `{connection_string}`"));

            {
                let mut stdout = io::stdout().lock();
                let _ = stdout.write(b"\nPress enter to kill database")?;
                stdout.flush()?;

                let mut buffer = String::new();
                let _ = io::stdin().read_line(&mut buffer);
            }
        }
    }

    Ok(())
}
