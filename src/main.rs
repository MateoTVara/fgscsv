use clap::Parser;

mod config;
mod cli;
mod media;
mod pipeline;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Init => {
            cli::init()?;
        },
        cli::Commands::Run { output } => {
            cli::run(output).await?;
        },
        cli::Commands::Set { key, value } => {
            cli::set(key, value)?;
        },
        cli::Commands::Path => {
            cli::path()?;
        },
        cli::Commands::Show => {
            cli::show()?;
        }
    }

    Ok(())
}
