mod baseline;
mod cases;
mod report;
mod sql;
mod workload;

use std::{path::PathBuf, process::ExitCode};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Runs required application queries against the performance database")]
struct Args {
    #[arg(long)]
    report_file: Option<PathBuf>,
    #[arg(long)]
    baseline_only: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let result = if args.baseline_only {
        baseline::run().await
    } else if let Some(report_file) = args.report_file {
        workload::run(&report_file).await
    } else {
        Err("--report-file is required unless --baseline-only is set".to_string())
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
