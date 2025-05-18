mod command;
mod run;
mod utils;

use anyhow::Result;
use clap::Parser;
use command::{Cli, Subcommands};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cli: Cli = Cli::parse();

    match &cli.subcommand {
        Subcommands::Run(run_args) => run::run(run_args).await,
    }
}
