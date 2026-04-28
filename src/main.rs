use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use hexis_cli::{Error, FileId, reconciler, supervisor};

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
    async fn run(self) -> Result<(), Error> {
        match self.command {
            Command::Apply { file, declared, dry_run: _ } => {
                // Spin up the actor topology so the wiring is exercised
                // even in v0.1, then surface NotYetImplemented because
                // the four-step chain isn't wired yet.
                let file_id = FileId::from_path(&file);
                let snapshot_dir = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/state/hexis/snapshot");
                let target = reconciler::Arguments {
                    file_id: file_id.clone(),
                    declared_path: declared,
                    live_path: file,
                    snapshot_dir,
                };
                let (sup, handle) = supervisor::Supervisor::start(supervisor::Arguments {
                    reconciler_targets: vec![target],
                })
                .await?;
                ractor::cast!(sup, supervisor::Message::Apply { file_id }).map_err(|error| {
                    Error::ActorCall {
                        label: "supervisor.apply",
                        reason: error.to_string(),
                    }
                })?;
                ractor::cast!(sup, supervisor::Message::Shutdown).map_err(|error| {
                    Error::ActorCall {
                        label: "supervisor.shutdown",
                        reason: error.to_string(),
                    }
                })?;
                drop(sup);
                let _ = handle.await;
                Err(Error::NotYetImplemented("apply"))
            }
            Command::Diff { .. } => Err(Error::NotYetImplemented("diff")),
            Command::Snapshot { .. } => Err(Error::NotYetImplemented("snapshot")),
            Command::Report => Err(Error::NotYetImplemented("report")),
            Command::Propose => Err(Error::NotYetImplemented("propose")),
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hexis: {error}");
            ExitCode::FAILURE
        }
    }
}
