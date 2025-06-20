use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
        /// Path to SQL file containing migrations to set up the database against.
        #[arg(short, long)]
        migrations: Option<PathBuf>,
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
