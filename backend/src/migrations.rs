use entrypoint_scheduler::GRAPHILE_WORKER_SCHEMA;
use graphile_worker::{Database, WorkerUtils};

pub async fn migrate_graphile_worker_schema(database: impl Into<Database>) -> Result<(), String> {
    WorkerUtils::new(database, GRAPHILE_WORKER_SCHEMA)
        .migrate()
        .await
        .map_err(|error| error.to_string())
}
