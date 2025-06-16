//! # SQL Helper
//! Split helper CLI for working with SQL.

mod cli;
mod operation;
mod operation_group;

use std::process::{Stdio, exit};

use clap::Parser;
use cli_helper::{Action, ActionResult, FileParser};
use color_eyre::eyre::eyre;

use crate::{
    cli::{Cli, run_tests},
    operation_group::OperationGroup,
};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    match cli.command {
        cli::Commands::Test { source } => {
            let action = Action::new("Parsing", "Parsed", source.to_string_lossy(), 0);
            let file_parser: FileParser<OperationGroup> =
                FileParser::parse(&source).bind_result(action)?;

            let result = run_tests(file_parser.modules);

            // Kill database
            let mut child = std::process::Command::new("docker")
                .args(["rm", "--force", "sql-helper-test-db"])
                .stdout(Stdio::null())
                .spawn()?;
            let status = child.wait()?;
            if !status.success() {
                return Err(eyre!("Killing test database failed with status: {status}"));
            }

            let tests_passed = result?;
            if !tests_passed {
                exit(-1);
            }
        }
        cli::Commands::GenerateBindings { source, target } => {
            let action = Action::new("Parsing", "Parsed", source.to_string_lossy(), 0);
            let file_parser: FileParser<OperationGroup> =
                FileParser::parse(&source).bind_result(action)?;

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

            file_parser.write(target.as_deref()).bind_result(action)?;
        }
    }

    Ok(())
}
