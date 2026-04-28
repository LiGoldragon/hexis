//! Hexis CLI entry point.
//!
//! v0.1 invokes [`Reconciler::apply`] directly for the single-shot
//! `hexis apply` command — no actor harness needed for one-target,
//! sync-IO work that completes in milliseconds. The supervisor /
//! reconciler / proposer actor topology exists for the v2
//! multi-target / watcher-driven flow; v0.1 exercises it through the
//! smoke test in `tests/scaffold.rs`.

use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use hexis_cli::live::Live;
use hexis_cli::snapshot::Snapshot;
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
            } => Self::run_apply(file, declared, dry_run),
            Command::Diff { file } => Self::run_diff(file),
            Command::Snapshot { .. } => Err(Error::NotYetImplemented("snapshot")),
            Command::Report => Self::run_report(),
            Command::Propose => Err(Error::NotYetImplemented("propose")),
        }
    }

    fn run_apply(file: PathBuf, declared: PathBuf, dry_run: bool) -> Result<(), Error> {
        let state = Self::state_dir();
        let arguments = reconciler::Arguments {
            file_id: FileId::from_path(&file),
            declared_path: declared,
            live_path: file,
            snapshot_dir: state.join("snapshot"),
            drift_dir: state.join("drift"),
            dry_run,
        };
        reconciler::Reconciler::apply(&arguments)
    }

    fn run_diff(file: PathBuf) -> Result<(), Error> {
        let file_id = FileId::from_path(&file);
        let snapshot_path = Self::state_dir()
            .join("snapshot")
            .join(format!("{file_id}.json"));
        if !snapshot_path.exists() {
            println!("(no snapshot for {file:?} — file has not been applied yet)");
            return Ok(());
        }
        let snapshot = Snapshot::from_path(&snapshot_path)?;
        let live = Live::from_path(&file)?;
        let drift = snapshot.drift_against(&live);
        let rendered = serde_json::to_string_pretty(drift.as_value())
            .expect("serde_json::Value always serializes");
        println!("{rendered}");
        Ok(())
    }

    fn run_report() -> Result<(), Error> {
        let drift_dir = Self::state_dir().join("drift");
        if !drift_dir.exists() {
            println!("(no drift reports yet)");
            return Ok(());
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(&drift_dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        entries.sort();
        if entries.is_empty() {
            println!("(no drift reports yet)");
            return Ok(());
        }
        for path in entries {
            Self::print_report_entry(&path)?;
        }
        Ok(())
    }

    fn print_report_entry(path: &Path) -> Result<(), Error> {
        let content = fs::read_to_string(path)?;
        let document: serde_json::Value = serde_json::from_str(&content).map_err(|error| {
            Error::DriftParse {
                source_path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        let file_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("<unknown>");
        println!("=== {file_id} ===");
        if let Some(applied_at) = document.pointer("/applied_at").and_then(|v| v.as_str()) {
            println!("applied_at: {applied_at}");
        }
        if let Some(drift) = document.pointer("/drift") {
            let rendered = serde_json::to_string_pretty(drift)
                .expect("serde_json::Value always serializes");
            println!("drift:\n{rendered}");
        }
        println!();
        Ok(())
    }

    /// `~/.local/state/hexis` for normal users; `/tmp/hexis-state` as a
    /// last-resort fallback when `HOME` is unset.
    fn state_dir() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".local/state/hexis"))
            .unwrap_or_else(|| PathBuf::from("/tmp/hexis-state"))
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
