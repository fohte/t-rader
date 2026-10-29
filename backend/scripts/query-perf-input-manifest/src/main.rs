use std::process::ExitCode;

use clap::Parser;

mod cli;
mod comparison;
mod constants;
mod manifest;
mod model;
#[path = "../../../crates/apps/query-perf/src/log.rs"]
mod query_log;
mod replay_exclusions;
mod runner;
mod util;

pub(crate) use constants::*;
pub(crate) use util::{hash_serialized, sha256, write_json};

use cli::{Args, BuildOptions, Command};
use manifest::build_manifest;
use replay_exclusions::verify_replay_exclusions;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();
    match args.command {
        Command::Build {
            log_file,
            database_url,
            target_date,
            source_checkout_sha,
            input_kind,
            expected_log_sha256,
            input_artifact_name,
            input_artifact_id,
            exclusions_file,
            output,
            group_binds_output,
            compare_to,
            comparison_output,
            failure_output,
        } => {
            build_manifest(BuildOptions {
                log_file,
                database_url,
                target_date,
                source_checkout_sha,
                input_kind,
                expected_log_sha256,
                input_artifact_name,
                input_artifact_id,
                exclusions_file,
                output,
                group_binds_output,
                compare_to,
                comparison_output,
                failure_output,
            })
            .await
        }
        Command::VerifyReplayExclusions {
            stdout,
            exclusions_file,
            output,
        } => verify_replay_exclusions(stdout, exclusions_file, output),
        Command::ReshiftGroupBinds {
            input,
            target_date,
            output,
        } => runner::write_profile_target_group_binds(&input, &target_date, &output),
    }
}
