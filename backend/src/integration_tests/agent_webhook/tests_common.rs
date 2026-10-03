use axum_test::TestServer;
use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::testing::create_test_server_with_state;

pub(crate) use crate::testing::TEST_AGENT_WEBHOOK_TOKEN;

pub(crate) const NOTIFICATION_TOKEN_HEADER: &str = "x-a2a-notification-token";

pub(crate) async fn build_server(db: gateway_postgres::DatabaseHandle) -> TestServer {
    create_test_server_with_state(db).await.1
}

pub(crate) async fn reconciliation_jobs(
    db: &gateway_postgres::DatabaseHandle,
) -> Vec<(String, String, String)> {
    let statement = Statement::from_string(
        DatabaseBackend::Postgres,
        format!(
            "SELECT tasks.identifier, jobs.payload::text AS payload, jobs.key \
             FROM graphile_worker._private_jobs AS jobs \
             JOIN graphile_worker._private_tasks AS tasks ON tasks.id = jobs.task_id \
             WHERE jobs.key = '{STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER}'"
        ),
    );
    db.query_all_raw(statement)
        .await
        .expect("read reconciliation jobs")
        .into_iter()
        .map(|row| {
            (
                row.try_get::<String>("", "identifier")
                    .expect("job identifier"),
                row.try_get::<String>("", "payload").expect("job payload"),
                row.try_get::<String>("", "key").expect("job key"),
            )
        })
        .collect()
}
