use futures_util::future::BoxFuture;
use graphile_worker::WorkerUtils;
use graphile_worker_admin_ui::{
    AdminAuthConfig, AdminServerConfig, build_router as build_admin_ui_router,
};
use sea_orm::DatabaseConnection;
use tokio::sync::watch;

use crate::{
    signals::wait_for_shutdown,
    startup::{StartupError, WorkerAdminUiAuth, WorkerAdminUiSettings},
};
use entrypoint_scheduler::GRAPHILE_WORKER_SCHEMA;

fn auth_config(header_name: String, header_value: String) -> Result<AdminAuthConfig, StartupError> {
    AdminAuthConfig::header(header_name, header_value, false).map_err(|error| {
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
    let auth = match settings.auth {
        WorkerAdminUiAuth::None => AdminAuthConfig::None,
        WorkerAdminUiAuth::Header { name, value } => auth_config(name, value)?,
    };
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

    const TEST_AUTH_HEADER_NAME: &str = "x-proxy-identity";
    const TEST_AUTH_HEADER_VALUE: &str = "authorized-user";

    fn create_test_server(
        auth: AdminAuthConfig,
        listen_addr: SocketAddr,
    ) -> Result<TestServer, Box<dyn Error>> {
        let pool = PgPool::connect_lazy("postgres://test:test@localhost/test")?;
        let config = AdminServerConfig::builder(
            pool.clone(),
            WorkerUtils::new(pool, GRAPHILE_WORKER_SCHEMA),
        )
        .schema_name(GRAPHILE_WORKER_SCHEMA)
        .listen_addr(listen_addr)
        .auth(auth)
        .read_only(false)
        .build()?;
        let router = build_admin_ui_router(config)?;

        Ok(TestServer::new(router)?)
    }

    #[fixture]
    fn header_auth_test_server() -> Result<TestServer, Box<dyn Error>> {
        create_test_server(
            auth_config(
                TEST_AUTH_HEADER_NAME.to_string(),
                TEST_AUTH_HEADER_VALUE.to_string(),
            )?,
            SocketAddr::from(([0, 0, 0, 0], 3001)),
        )
    }

    #[fixture]
    fn loopback_test_server() -> Result<TestServer, Box<dyn Error>> {
        create_test_server(
            AdminAuthConfig::None,
            SocketAddr::from(([127, 0, 0, 1], 3001)),
        )
    }

    #[rstest]
    #[case::matching_proxy_header_without_ui_token(Some(TEST_AUTH_HEADER_VALUE), StatusCode::OK)]
    #[case::missing_proxy_header(None, StatusCode::UNAUTHORIZED)]
    #[case::different_proxy_header(Some("other-user"), StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn api_session_requires_the_configured_proxy_header(
        header_auth_test_server: Result<TestServer, Box<dyn Error>>,
        #[case] header_value: Option<&str>,
        #[case] expected_status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        let server = header_auth_test_server?;
        let request = server.get("/api/session");
        let response = match header_value {
            Some(header_value) => {
                request
                    .add_header(TEST_AUTH_HEADER_NAME, header_value)
                    .await
            }
            None => request.await,
        };

        assert_eq!(response.status_code(), expected_status);
        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn loopback_without_auth_serves_api_session(
        loopback_test_server: Result<TestServer, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let server = loopback_test_server?;
        let response = server.get("/api/session").await;

        assert_eq!(response.status_code(), StatusCode::OK);
        Ok(())
    }
}
