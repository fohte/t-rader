mod log;
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
use runner::PerfReport;

const EXCLUSIONS: &str = include_str!("../exclusions.json");

#[derive(Debug, Parser)]
#[command(about = "Replays backend test SELECT statements against the performance database")]
struct Args {
    #[arg(long)]
    log_file: PathBuf,
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
    let exclusions: Vec<Exclusion> = serde_json::from_str(EXCLUSIONS)?;
    validate_exclusions(&exclusions)?;

    let parsed_log = log::parse_json_log(&log_contents, &exclusions).map_err(io::Error::other)?;
    let report = runner::replay(&database_url, parsed_log, exclusions).await?;
    print_report(&report);
    append_step_summary(&report)?;

    if report.failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "query-perf found {} failing SQL statement(s)",
            report.failures.len()
        ))
        .into())
    }
}

fn validate_exclusions(exclusions: &[Exclusion]) -> io::Result<()> {
    if let Some(exclusion) = exclusions
        .iter()
        .find(|exclusion| exclusion.sql.trim().is_empty() || exclusion.reason.trim().is_empty())
    {
        return Err(io::Error::other(format!(
            "query-perf exclusions require a SQL statement and reason: {exclusion:?}"
        )));
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
            "{} max={}ms runs={} SQL={}",
            if statement.succeeded { "PASS" } else { "FAIL" },
            statement.max_duration.as_secs_f64() * 1_000.0,
            statement.runs,
            statement.sql.replace('\n', " ")
        );
    }
    for excluded in &report.excluded {
        println!("EXCLUDED reason={} SQL={}", excluded.reason, excluded.sql);
    }
    for failure in &report.failures {
        eprintln!(
            indoc! {"
                FAIL {} after {}ms
                SQL: {}
                {}: {}
                {}
            "},
            failure.kind,
            failure.duration.as_secs_f64() * 1_000.0,
            failure.sql,
            if failure.shifted_parameters {
                "Shifted parameters"
            } else {
                "Original parameters"
            },
            failure.parameters,
            failure.message
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

            | Result | Maximum | Runs | SQL |
            | --- | ---: | ---: | --- |
        "},
        report.target_date
    )?;
    for statement in &report.statements {
        writeln!(
            file,
            "| {} | {:.3} ms | {} | `{}` |",
            if statement.succeeded { "PASS" } else { "FAIL" },
            statement.max_duration.as_secs_f64() * 1_000.0,
            statement.runs,
            statement.sql.replace('|', "\\|").replace('\n', " ")
        )?;
    }
    for excluded in &report.excluded {
        writeln!(
            file,
            "\nExcluded `{}`: {}",
            excluded.sql.replace('`', "\\`").replace('\n', " "),
            excluded.reason
        )?;
    }
    for failure in &report.failures {
        writeln!(file)?;
        writeln!(
            file,
            indoc! {"
                ### Failed SQL ({})

                Elapsed: {:.3} ms

                SQL:

                ```sql
                {}
                ```

                {}: `{}`

                {}
            "},
            failure.kind,
            failure.duration.as_secs_f64() * 1_000.0,
            failure.sql,
            if failure.shifted_parameters {
                "Shifted parameters"
            } else {
                "Original parameters"
            },
            failure.parameters.replace('`', "\\`"),
            failure.message
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
            Err("query-perf exclusions require a SQL statement and reason: Exclusion { sql: \"SELECT 1\", reason: \"\" }".to_string())
        );
    }
}
