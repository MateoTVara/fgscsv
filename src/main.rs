// keep-sorted start
mod cli;
mod config;
mod dsl;
mod pipeline;
use clap::Parser;
// keep-sorted end

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Run => cli::run_cli().await?,
        cli::Commands::Init => cli::init_cli()?,
    };

    Ok(())
}
