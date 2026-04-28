use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use hexis_cli::Error;

#[derive(Parser)]
#[command(
    name = "hexis",
    version,
    about = "Managed-mutable config reconciliation with per-key modes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply the declared overlay to the live file, writing snapshot + drift report.
    Apply {
        /// Path to the live config file.
        #[arg(long)]
        file: PathBuf,
        /// Path to the declared overlay JSON.
        #[arg(long)]
        declared: PathBuf,
        /// Print the drift patch and proposed new live without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the user drift relative to the snapshot, without applying.
    Diff {
        #[arg(long)]
        file: PathBuf,
    },
    /// Capture the current live file as a new snapshot baseline.
    Snapshot {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        to: PathBuf,
    },
    /// Print accumulated drift across all managed files.
    Report,
    /// Run the proposal loop now (v2+ — currently a no-op).
    Propose,
}

impl Cli {
    fn run(self) -> Result<(), Error> {
        match self.command {
            Command::Apply { .. } => Err(Error::NotYetImplemented("apply")),
            Command::Diff { .. } => Err(Error::NotYetImplemented("diff")),
            Command::Snapshot { .. } => Err(Error::NotYetImplemented("snapshot")),
            Command::Report => Err(Error::NotYetImplemented("report")),
            Command::Propose => Err(Error::NotYetImplemented("propose")),
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hexis: {error}");
            ExitCode::FAILURE
        }
    }
}
