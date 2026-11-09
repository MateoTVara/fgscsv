// keep-sorted start
use clap::{Parser, Subcommand};
// keep-sorted end

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Fetch and process spreadsheet data from the web according to the configuration.
    Run,

    /// Create an initial configuration file.
    Init,
}

#[derive(Debug, Parser)]
#[command(
    name = "fgscsv",
    version,
    about = "Sync CSV spreadsheets into typed JSON using a schema DSL."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run() {
        let cli = Cli::try_parse_from(["fgscsv", "run"]).unwrap();
        assert!(matches!(cli.command, Commands::Run));
    }

    #[test]
    fn parses_init() {
        let cli = Cli::try_parse_from(["fgscsv", "init"]).unwrap();
        assert!(matches!(cli.command, Commands::Init));
    }

    #[test]
    fn rejects_unknown_command() {
        assert!(Cli::try_parse_from(["fgscsv", "sync"]).is_err());
    }

    #[test]
    fn requires_subcommand() {
        assert!(Cli::try_parse_from(["fgscsv"]).is_err());
    }
}
