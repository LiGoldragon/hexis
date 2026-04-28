//! Hexis CLI entry point.
//!
//! v0.1 invokes [`Reconciler::apply`] directly for the single-shot
//! `hexis apply` command — no actor harness needed for one-target,
//! sync-IO work that completes in milliseconds. The supervisor /
//! reconciler / proposer actor topology exists for the v2
//! multi-target / watcher-driven flow; v0.1 exercises it through the
//! smoke test in `tests/scaffold.rs`.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use hexis_cli::{Error, FileId, reconciler};

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
        /// Run the four-step apply but skip the writes.
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
            Command::Apply {
                file,
                declared,
                dry_run,
            } => {
                let arguments = reconciler::Arguments {
                    file_id: FileId::from_path(&file),
                    declared_path: declared,
                    live_path: file,
                    snapshot_dir: state_dir().join("snapshot"),
                    drift_dir: state_dir().join("drift"),
                    dry_run,
                };
                reconciler::Reconciler::apply(&arguments)
            }
            Command::Diff { .. } => Err(Error::NotYetImplemented("diff")),
            Command::Snapshot { .. } => Err(Error::NotYetImplemented("snapshot")),
            Command::Report => Err(Error::NotYetImplemented("report")),
            Command::Propose => Err(Error::NotYetImplemented("propose")),
        }
    }
}

/// `~/.local/state/hexis` for normal users; `/tmp/hexis-state` as a
/// last-resort fallback when `HOME` is unset.
fn state_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local/state/hexis"))
        .unwrap_or_else(|| PathBuf::from("/tmp/hexis-state"))
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
