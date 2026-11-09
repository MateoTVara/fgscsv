// keep-sorted start
use clap::Parser;
use fgscsv::{
    app,
    cli::{Cli, Commands},
};
use std::process::ExitCode;
// keep-sorted end

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run => app::run().await,
        Commands::Init => app::init(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
