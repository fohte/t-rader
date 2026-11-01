use std::{
    collections::BTreeMap,
    env,
    path::Path,
    time::{Duration, Instant},
};

use backend::services::use_cases::build_use_cases_without_agent_task_client;
use chrono::{Days, NaiveDate};
use core_application::{
    group_movers::{GroupMoversQuery, GroupMoversUseCaseError},
    widget::{WidgetQueryResponse, WidgetUseCaseError},
};
use gateway_postgres::DatabaseHandle;
use rust_decimal::Decimal;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseBackend, Statement};

use crate::{
    cases::{
        WorkloadCase, build_cases, missing_target_group_definition, reference_date_definition,
        required_case_ids,
    },
    report::{WorkloadReport, write_report},
    sql::{expected_sql_by_case, fingerprint, required_sql_fingerprints},
};

const EXPECTED_WIDGET_CASE_COUNT: usize = 296;
const GROUP_MOVER_CASES: [(&str, i64, bool); 4] = [
    ("group-movers-short", 5, false),
    ("group-movers-long", 60, false),
    ("group-movers-short-filtered", 5, true),
    ("group-movers-long-filtered", 60, true),
];
const GROUPS_PER_AXIS: usize = 30;
const GROUP_MEMBERS: i64 = 500;
const TOP_STOCKS_PER_GROUP: usize = 5;

pub(super) async fn run(report_path: &Path) -> Result<(), String> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required for the performance workload".to_string())?;
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1);
    options.min_connections(1);
    options.connect_timeout(Duration::from_secs(30));
    options.sqlx_logging(false);
    options.map_sqlx_postgres_opts(|options| options.statement_cache_capacity(512));
    let connection = Database::connect(options)
        .await
        .map_err(|_| "could not connect to the performance database".to_string())?;
    let database = DatabaseHandle::from(connection.clone());
    connection
        .execute_unprepared("SET TIME ZONE 'UTC'")
        .await
        .map_err(|_| "could not set the performance database timezone".to_string())?;
    configure_read_only_session(&connection).await?;
    let use_cases = build_use_cases_without_agent_task_client(database);

    set_application_name(&connection, "workload-bootstrap").await?;
    let reference_response = use_cases
        .widgets()
        .query(reference_date_definition())
        .await
        .map_err(widget_error)?;
    let reference_date = reference_response.as_of;
    verify_reference_response(&reference_response, reference_date)?;
    verify_missing_group_rejected(&use_cases, reference_date).await?;
    verify_missing_target_group_rejected(&connection, &use_cases, reference_date).await?;

    let cases = build_cases(reference_date)?;
    if cases.len() != EXPECTED_WIDGET_CASE_COUNT {
        return Err("widget case matrix is incomplete".to_string());
    }
    let required_ids = required_case_ids(&cases);
    let definitions = cases
        .iter()
        .map(|case| (case.id.as_str(), &case.definition))
        .collect::<Vec<_>>();
    let expected_sql_by_case = expected_sql_by_case(&definitions)?;
    let mut completed = Vec::with_capacity(required_ids.len());

    for case in &cases {
        run_widget_case(&connection, &use_cases, case).await?;
        completed.push(case.id.clone());
    }
    for (case_id, days, filtered) in GROUP_MOVER_CASES {
        run_group_mover_case(
            &connection,
            &use_cases,
            reference_date,
            case_id,
            days,
            filtered,
        )
        .await?;
        completed.push(case_id.to_string());
    }

    verify_prepared_statement_reuse(&connection).await?;

    let report = WorkloadReport {
        required_case_ids: required_ids,
        completed_case_ids: completed,
        expected_sql_by_case,
    };
    write_report(report_path, &report)?;
    println!(
        "Completed {} required cases against profile reference date {}.",
        report.completed_case_ids.len(),
        reference_date
    );
    Ok(())
}

async fn configure_read_only_session(
    connection: &sea_orm::DatabaseConnection,
) -> Result<(), String> {
    connection
        .execute_unprepared("SET default_transaction_read_only = on")
        .await
        .map_err(|_| "could not make the performance connection read-only".to_string())?;
    connection
        .execute_unprepared("SET statement_timeout = '5s'")
        .await
        .map_err(|_| "could not apply the five second SQL limit".to_string())?;
    connection
        .execute_unprepared("SET plan_cache_mode = auto")
        .await
        .map_err(|_| "could not enable automatic prepared statement planning".to_string())?;
    Ok(())
}

async fn set_application_name(
    connection: &sea_orm::DatabaseConnection,
    application_name: &str,
) -> Result<(), String> {
    if !application_name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("invalid performance application name".to_string());
    }
    let value = if application_name.starts_with("widget-")
        || application_name.starts_with("group-movers-")
    {
        format!("perf:{application_name}")
    } else {
        application_name.to_string()
    };
    connection
        .execute_unprepared(&format!("SET application_name = '{value}'"))
        .await
        .map_err(|_| "could not set the workload application name".to_string())?;
    Ok(())
}

async fn run_widget_case(
    connection: &sea_orm::DatabaseConnection,
    use_cases: &backend::services::use_cases::UseCases,
    case: &WorkloadCase,
) -> Result<(), String> {
    let started = Instant::now();
    set_application_name(connection, &case.id).await?;
    let actual = use_cases
        .widgets()
        .query(case.definition.clone())
        .await
        .map_err(|_| format!("widget case {} failed", case.id))?;
    verify_widget_output(case, &actual)?;
    let repeated = use_cases
        .widgets()
        .query(case.definition.clone())
        .await
        .map_err(|_| format!("widget case {} failed on repeat", case.id))?;
    if actual != repeated {
        return Err(format!(
            "widget case {} response changed on repeat",
            case.id
        ));
    }
    println!(
        "CASE {} elapsed_ms={}",
        case.id,
        started.elapsed().as_millis()
    );
    Ok(())
}

fn verify_widget_output(case: &WorkloadCase, response: &WidgetQueryResponse) -> Result<(), String> {
    if response.as_of != case.expected_as_of {
        return Err(format!(
            "widget case {} resolved an unexpected date",
            case.id
        ));
    }
    if response.rows.is_empty() {
        return Err(format!(
            "widget case {} returned no candidate rows",
            case.id
        ));
    }
    if response.rows.len() != case.expected_row_count || response.series.is_some() {
        return Err(format!(
            "widget case {} response did not match its expected shape",
            case.id
        ));
    }
    if case.expected_candidate_count < 8
        || (case.full_candidate_response && response.rows.len() != case.expected_candidate_count)
        || response
            .rows
            .iter()
            .any(|row| !matches!(row.values.get("value"), Some(Some(_))))
    {
        return Err(format!(
            "widget case {} had insufficient candidate data",
            case.id
        ));
    }
    Ok(())
}

async fn run_group_mover_case(
    connection: &sea_orm::DatabaseConnection,
    use_cases: &backend::services::use_cases::UseCases,
    reference_date: NaiveDate,
    case_id: &str,
    days: i64,
    filtered: bool,
) -> Result<(), String> {
    let from = reference_date
        .checked_sub_days(Days::new(days as u64))
        .ok_or_else(|| "reference date is outside the supported range".to_string())?;
    let query = GroupMoversQuery {
        axis_key: "demo-axis".to_string(),
        from,
        to: reference_date,
        min_avg_turnover: if filtered {
            Decimal::new(10_000_000, 0)
        } else {
            Decimal::ZERO
        },
    };
    set_application_name(connection, case_id).await?;
    let started = Instant::now();
    let mut expected_response = None;
    for _ in 0..6 {
        let response = use_cases
            .group_movers()
            .list_group_movers(query.clone())
            .await
            .map_err(|_| format!("group movers case {case_id} failed"))?;
        verify_group_mover_response(&response, filtered, case_id)?;
        match &expected_response {
            Some(expected) if expected != &response => {
                return Err(format!(
                    "group movers case {case_id} response changed on repeat"
                ));
            }
            None => expected_response = Some(response),
            Some(_) => {}
        }
    }
    println!(
        "CASE {case_id} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn verify_group_mover_response(
    response: &[core_application::group_movers::GroupMover],
    filtered: bool,
    case_id: &str,
) -> Result<(), String> {
    let eligible_count = response
        .iter()
        .map(|group| group.constituent_count)
        .sum::<i64>();
    if response.len() != GROUPS_PER_AXIS
        || eligible_count <= 0
        || response.iter().any(|group| {
            group.constituent_count <= 0
                || group.median_change_rate.is_none()
                || group.turnover_weighted_change_rate.is_none()
                || group.total_avg_turnover <= Decimal::ZERO
                || group.top_stocks.len() != TOP_STOCKS_PER_GROUP
        })
        || (!filtered
            && response
                .iter()
                .any(|group| group.constituent_count != GROUP_MEMBERS))
    {
        return Err(format!(
            "group movers case {case_id} did not return the expected groups"
        ));
    }
    Ok(())
}

async fn verify_missing_group_rejected(
    use_cases: &backend::services::use_cases::UseCases,
    reference_date: NaiveDate,
) -> Result<(), String> {
    let from = reference_date
        .checked_sub_days(Days::new(5))
        .ok_or_else(|| "reference date is outside the supported range".to_string())?;
    let result = use_cases
        .group_movers()
        .list_group_movers(GroupMoversQuery {
            axis_key: "missing-axis".to_string(),
            from,
            to: reference_date,
            min_avg_turnover: Decimal::ZERO,
        })
        .await;
    match result {
        Err(GroupMoversUseCaseError::AxisNotFound(_)) => {
            println!("Verified missing group axis fails at the application boundary.");
            Ok(())
        }
        _ => Err("missing group axis did not fail with AxisNotFound".to_string()),
    }
}

async fn verify_missing_target_group_rejected(
    connection: &sea_orm::DatabaseConnection,
    use_cases: &backend::services::use_cases::UseCases,
    reference_date: NaiveDate,
) -> Result<(), String> {
    let case = WorkloadCase {
        id: "missing-target-group-probe".to_string(),
        definition: missing_target_group_definition(reference_date),
        expected_candidate_count: GROUP_MEMBERS as usize,
        expected_row_count: TOP_STOCKS_PER_GROUP,
        expected_as_of: reference_date,
        full_candidate_response: false,
    };
    set_application_name(connection, &case.id).await?;
    let response = use_cases
        .widgets()
        .query(case.definition.clone())
        .await
        .map_err(|_| "missing target group probe could not execute".to_string())?;
    match verify_widget_output(&case, &response) {
        Err(error) if error.contains("returned no candidate rows") => {
            println!("Verified an absent target group is rejected for insufficient candidates.");
            Ok(())
        }
        _ => Err("missing target group did not fail for insufficient candidates".to_string()),
    }
}

async fn verify_prepared_statement_reuse(
    connection: &sea_orm::DatabaseConnection,
) -> Result<(), String> {
    let mode = connection
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_setting('plan_cache_mode') AS plan_cache_mode".to_string(),
        ))
        .await
        .map_err(|_| "could not inspect prepared statement planning mode".to_string())?
        .ok_or_else(|| "prepared statement planning mode was unavailable".to_string())?
        .try_get::<String>("", "plan_cache_mode")
        .map_err(|_| "could not read prepared statement planning mode".to_string())?;
    if mode != "auto" {
        return Err("prepared statement planning mode was not auto".to_string());
    }

    let signatures = required_sql_fingerprints()?;
    let rows = connection
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT statement, generic_plans, custom_plans FROM pg_prepared_statements".to_string(),
        ))
        .await
        .map_err(|_| "could not inspect prepared statement reuse".to_string())?;
    let mut observations = BTreeMap::new();
    for row in rows {
        let sql = row
            .try_get::<String>("", "statement")
            .map_err(|_| "could not read a prepared statement".to_string())?;
        let generic = row
            .try_get::<i64>("", "generic_plans")
            .map_err(|_| "could not read generic plan count".to_string())?;
        let custom = row
            .try_get::<i64>("", "custom_plans")
            .map_err(|_| "could not read custom plan count".to_string())?;
        observations.insert(fingerprint(&sql), (generic, custom));
    }
    for (label, sql_name) in [
        ("widget query", "widget.query"),
        ("group movers", "group-movers.list"),
    ] {
        let expected = signatures
            .get(sql_name)
            .ok_or_else(|| format!("missing {label} SQL signature"))?;
        let (generic, custom) = observations
            .get(expected)
            .copied()
            .ok_or_else(|| format!("{label} prepared statement was not retained"))?;
        let executions = generic + custom;
        if executions < 6 {
            return Err(format!(
                "{label} prepared statement was reused fewer than six times"
            ));
        }
        println!(
            "PLAN {label} executions={executions} generic={generic} custom={custom} mode=auto"
        );
    }
    Ok(())
}

fn verify_reference_response(
    response: &WidgetQueryResponse,
    reference_date: NaiveDate,
) -> Result<(), String> {
    if response.as_of != reference_date || response.rows.len() != 1 {
        return Err("could not resolve the performance profile reference date".to_string());
    }
    Ok(())
}

fn widget_error(_: WidgetUseCaseError) -> String {
    "could not resolve the performance profile reference date".to_string()
}
