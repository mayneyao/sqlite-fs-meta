mod args;
mod commands;
mod config;
mod error;
mod output;
mod query;
mod schema_command;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = args::Cli::parse();
    match commands::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            output::print_error(&error);
            ExitCode::from(error.exit_code())
        }
    }
}
