use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Creates and compares hash-only query-perf input manifests")]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Build {
        #[arg(long)]
        log_file: PathBuf,
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        target_date: String,
        #[arg(long)]
        source_checkout_sha: String,
        #[arg(long, default_value = "ci-regenerated")]
        input_kind: String,
        #[arg(long)]
        expected_log_sha256: Option<String>,
        #[arg(long)]
        input_artifact_name: Option<String>,
        #[arg(long)]
        input_artifact_id: Option<String>,
        #[arg(long)]
        exclusions_file: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        group_binds_output: Option<PathBuf>,
        #[arg(long)]
        compare_to: Option<PathBuf>,
        #[arg(long)]
        comparison_output: Option<PathBuf>,
        #[arg(long)]
        failure_output: Option<PathBuf>,
    },
    VerifyReplayExclusions {
        #[arg(long)]
        stdout: PathBuf,
        #[arg(long)]
        exclusions_file: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    ReshiftGroupBinds {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        target_date: String,
        #[arg(long)]
        output: PathBuf,
    },
}

pub(crate) struct BuildOptions {
    pub(crate) log_file: PathBuf,
    pub(crate) database_url: String,
    pub(crate) target_date: String,
    pub(crate) source_checkout_sha: String,
    pub(crate) input_kind: String,
    pub(crate) expected_log_sha256: Option<String>,
    pub(crate) input_artifact_name: Option<String>,
    pub(crate) input_artifact_id: Option<String>,
    pub(crate) exclusions_file: PathBuf,
    pub(crate) output: PathBuf,
    pub(crate) group_binds_output: Option<PathBuf>,
    pub(crate) compare_to: Option<PathBuf>,
    pub(crate) comparison_output: Option<PathBuf>,
    pub(crate) failure_output: Option<PathBuf>,
}
