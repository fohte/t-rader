mod log;
mod required_workload;
mod runner;

use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::Parser;
use indoc::indoc;
use log::Exclusion;
use required_workload::{
    WorkloadFailureKind, parse_workload_report, sql_fingerprint, verify_required_workload,
};
use runner::PerfReport;

const EXCLUSIONS: &str = include_str!("../exclusions.json");
const EXPECTED_REQUIRED_CASE_COUNT: usize = 300;

#[derive(Debug, Parser)]
#[command(
    about = "Replays backend test SELECT statements and verifies required workload observations"
)]
struct Args {
    #[arg(long)]
    log_file: PathBuf,
    #[arg(long)]
    workload_log: PathBuf,
    #[arg(long)]
    workload_report: PathBuf,
}

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

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = Args::parse();
    let database_url = env::var("DATABASE_URL")?;
    let log_contents = fs::read_to_string(args.log_file)?;
    let workload_log = fs::read_to_string(args.workload_log)?;
    let workload_report = fs::read_to_string(args.workload_report)?;
    let exclusions: Vec<Exclusion> = serde_json::from_str(EXCLUSIONS)?;
    validate_exclusions(&exclusions)?;

    let parsed_log = log::parse_json_log(&log_contents, &exclusions)
        .map_err(|_| io::Error::other("query-perf could not parse the backend SQL log"))?;
    let workload_report =
        parse_workload_report(&workload_report).map_err(workload_failure_error)?;
    let excluded_fingerprints = exclusions
        .iter()
        .map(|exclusion| sql_fingerprint(&exclusion.sql))
        .collect();
    let workload_verification =
        verify_required_workload(&workload_report, &workload_log, &excluded_fingerprints)
            .map_err(workload_failure_error)?;
    if workload_verification.case_count != EXPECTED_REQUIRED_CASE_COUNT {
        return Err(io::Error::other(format!(
            "required workload failed: expected {EXPECTED_REQUIRED_CASE_COUNT} cases, observed {}",
            workload_verification.case_count
        ))
        .into());
    }
    let report = runner::replay(&database_url, parsed_log, exclusions)
        .await
        .map_err(|_| io::Error::other("query-perf could not replay the backend SQL log"))?;
    print_report(&report);
    println!(
        "Verified required workload: cases={} expected SQL statements={}",
        workload_verification.case_count, workload_verification.observed_sql_count
    );
    append_step_summary(&report)?;

    if report.failures.is_empty() && !report.statements.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "query-perf replay failed: failures={}, replayed statements={}",
            report.failures.len(),
            report.statements.len()
        ))
        .into())
    }
}

fn workload_failure_error(kind: WorkloadFailureKind) -> io::Error {
    let reason = match kind {
        WorkloadFailureKind::InvalidReport => "invalid workload report",
        WorkloadFailureKind::InvalidSqlLog => "invalid workload SQL log",
        WorkloadFailureKind::DuplicateCaseId => "duplicate workload case ID",
        WorkloadFailureKind::MissingCase => "required workload case did not complete",
        WorkloadFailureKind::UnexpectedCompletedCase => "unexpected workload case completed",
        WorkloadFailureKind::MissingExpectation => "workload case has no expected SQL",
        WorkloadFailureKind::UnexpectedExpectation => {
            "workload report has an unexpected SQL expectation"
        }
        WorkloadFailureKind::DuplicateExpectation => "workload SQL expectation is duplicated",
        WorkloadFailureKind::MissingObservation => "expected workload SQL was not observed",
        WorkloadFailureKind::UnexpectedObservedCase => {
            "unexpected workload case appeared in SQL log"
        }
        WorkloadFailureKind::ExcludedOnly => "required workload SQL was handled only by exclusions",
    };
    io::Error::other(format!("required workload failed: {reason}"))
}

fn validate_exclusions(exclusions: &[Exclusion]) -> io::Result<()> {
    if exclusions
        .iter()
        .any(|exclusion| exclusion.sql.trim().is_empty() || exclusion.reason.trim().is_empty())
    {
        return Err(io::Error::other(
            "query-perf exclusions require a SQL statement and reason",
        ));
    }

    Ok(())
}

fn print_report(report: &PerfReport) {
    println!(
        "Replayed {} SQL statement(s) against data through {}.",
        report.statements.len(),
        report.target_date
    );
    for statement in &report.statements {
        println!(
            "{} max={}ms runs={} SQL fingerprint={} replay fingerprint={}",
            if statement.succeeded { "PASS" } else { "FAIL" },
            statement.max_duration.as_secs_f64() * 1_000.0,
            statement.runs,
            sql_fingerprint(&statement.sql),
            sql_fingerprint(&statement.replay_sql)
        );
    }
    for excluded in &report.excluded {
        println!(
            "EXCLUDED reason={} SQL fingerprint={}",
            excluded.reason,
            sql_fingerprint(&excluded.sql)
        );
    }
    for failure in &report.failures {
        let category = if failure.message.to_ascii_lowercase().contains("timeout") {
            "timeout"
        } else {
            "execution"
        };
        eprintln!(
            "FAIL {} category={} after {}ms SQL fingerprint={} replay fingerprint={} shifted_parameters={} has_parameters={}",
            failure.kind,
            category,
            failure.duration.as_secs_f64() * 1_000.0,
            sql_fingerprint(&failure.sql),
            sql_fingerprint(&failure.replay_sql),
            failure.shifted_parameters,
            !failure.parameters.is_empty()
        );
    }
}

fn append_step_summary(report: &PerfReport) -> io::Result<()> {
    let Some(path) = env::var_os("GITHUB_STEP_SUMMARY") else {
        return Ok(());
    };

    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    writeln!(
        file,
        indoc! {"
            ## Query performance

            Target date: `{}`

            | Result | Maximum | Runs | SQL fingerprint | Replay fingerprint |
            | --- | ---: | ---: | --- | --- |
        "},
        report.target_date
    )?;
    for statement in &report.statements {
        writeln!(
            file,
            "| {} | {:.3} ms | {} | `{}` | `{}` |",
            if statement.succeeded { "PASS" } else { "FAIL" },
            statement.max_duration.as_secs_f64() * 1_000.0,
            statement.runs,
            sql_fingerprint(&statement.sql),
            sql_fingerprint(&statement.replay_sql)
        )?;
    }
    for excluded in &report.excluded {
        writeln!(
            file,
            "\nExcluded SQL fingerprint `{}`: {}",
            sql_fingerprint(&excluded.sql),
            excluded.reason
        )?;
    }
    for failure in &report.failures {
        writeln!(file)?;
        writeln!(
            file,
            indoc! {"
                ### Failed SQL

                Kind: `{}`
                Elapsed: {:.3} ms
                Fingerprint: `{}`
            "},
            failure.kind,
            failure.duration.as_secs_f64() * 1_000.0,
            sql_fingerprint(&failure.sql)
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_exclusions;
    use crate::log::Exclusion;

    #[test]
    fn exclusion_requires_a_reason_and_a_statement() {
        assert_eq!(
            validate_exclusions(&[Exclusion {
                sql: "SELECT 1".to_string(),
                reason: String::new(),
            }])
            .map_err(|error| error.to_string()),
            Err("query-perf exclusions require a SQL statement and reason".to_string())
        );
    }
}
