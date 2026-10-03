use futures_util::future::BoxFuture;
use graphile_worker::WorkerUtils;
use graphile_worker_admin_ui::{
    AdminAuthConfig, AdminServerConfig, build_router as build_admin_ui_router,
};
use sea_orm::DatabaseConnection;
use tokio::sync::watch;

use crate::{
    signals::wait_for_shutdown,
    startup::{StartupError, WorkerAdminUiSettings},
};
use entrypoint_scheduler::GRAPHILE_WORKER_SCHEMA;

const ACCESS_HEADER_NAME: &str = "Cf-Access-Authenticated-User-Email";

fn auth_config(allowed_email: String) -> Result<AdminAuthConfig, StartupError> {
    AdminAuthConfig::header(ACCESS_HEADER_NAME, allowed_email, false).map_err(|error| {
        StartupError::Config(format!(
            "invalid Graphile Worker admin UI auth header: {error}"
        ))
    })
}

pub(super) async fn server(
    settings: WorkerAdminUiSettings,
    db: &DatabaseConnection,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<BoxFuture<'static, Result<(), StartupError>>, StartupError> {
    let pool = db.get_postgres_connection_pool().clone();
    let auth = auth_config(settings.allowed_email)?;
    let admin_config =
        AdminServerConfig::builder(pool.clone(), WorkerUtils::new(pool, GRAPHILE_WORKER_SCHEMA))
            .schema_name(GRAPHILE_WORKER_SCHEMA)
            .listen_addr(settings.listen_addr)
            .auth(auth)
            .read_only(false)
            .build()
            .map_err(|error| {
                StartupError::Config(format!(
                    "failed to configure Graphile Worker admin UI: {error}"
                ))
            })?;
    let admin_router = build_admin_ui_router(admin_config).map_err(|error| {
        StartupError::Config(format!("failed to build Graphile Worker admin UI: {error}"))
    })?;
    let listener = tokio::net::TcpListener::bind(settings.listen_addr)
        .await
        .map_err(|error| {
            StartupError::Config(format!(
                "failed to bind Graphile Worker admin UI to {}: {error}",
                settings.listen_addr
            ))
        })?;
    tracing::info!(
        "Graphile Worker admin UI listening on {}",
        settings.listen_addr
    );

    let admin_shutdown = wait_for_shutdown(shutdown_rx.clone());
    let admin_shutdown_receiver = shutdown_rx;
    Ok(Box::pin(async move {
        axum::serve(listener, admin_router)
            .with_graceful_shutdown(admin_shutdown)
            .await
            .map_err(|error| {
                StartupError::Runtime(format!("Graphile Worker admin UI server error: {error}"))
            })?;
        if !*admin_shutdown_receiver.borrow() {
            return Err(StartupError::Runtime(
                "Graphile Worker admin UI server stopped unexpectedly".to_string(),
            ));
        }
        Ok(())
    }))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::net::SocketAddr;

    use axum::http::StatusCode;
    use axum_test::TestServer;
    use rstest::{fixture, rstest};
    use sqlx::PgPool;

    use super::*;

    const TEST_ALLOWED_EMAIL: &str = "worker-admin@access.invalid";

    #[fixture]
    fn admin_ui_test_server() -> Result<TestServer, Box<dyn Error>> {
        let pool = PgPool::connect_lazy("postgres://test:test@localhost/test")?;
        let config = AdminServerConfig::builder(
            pool.clone(),
            WorkerUtils::new(pool, GRAPHILE_WORKER_SCHEMA),
        )
        .schema_name(GRAPHILE_WORKER_SCHEMA)
        .listen_addr(SocketAddr::from(([0, 0, 0, 0], 3001)))
        .auth(auth_config(TEST_ALLOWED_EMAIL.to_string())?)
        .read_only(false)
        .build()?;
        let router = build_admin_ui_router(config)?;

        Ok(TestServer::new(router)?)
    }

    #[rstest]
    #[case::allowed_identity(Some(TEST_ALLOWED_EMAIL), StatusCode::OK)]
    #[case::missing_identity(None, StatusCode::UNAUTHORIZED)]
    #[case::different_identity(Some("other-worker@access.invalid"), StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn api_session_requires_the_configured_identity(
        admin_ui_test_server: Result<TestServer, Box<dyn Error>>,
        #[case] identity: Option<&str>,
        #[case] expected_status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        let server = admin_ui_test_server?;
        let request = server.get("/api/session");
        let response = match identity {
            Some(identity) => request.add_header(ACCESS_HEADER_NAME, identity).await,
            None => request.await,
        };

        assert_eq!(response.status_code(), expected_status);
        Ok(())
    }
}
