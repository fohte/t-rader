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
    use graphile_worker_admin_ui::PublicAuthMode;

    use super::*;

    #[test]
    fn auth_uses_cloudflare_access_identity_header() {
        let allowed_email = "worker-admin@access.invalid";
        let actual = auth_config(allowed_email.to_string())
            .map(|auth| {
                let summary = auth.summary();
                (
                    matches!(summary.mode, PublicAuthMode::Header),
                    summary.header_name,
                    auth.secret_for_display().map(str::to_owned),
                    summary.generated_secret,
                )
            })
            .map_err(|error| error.to_string());

        assert_eq!(
            actual,
            Ok((
                true,
                Some("cf-access-authenticated-user-email".to_string()),
                Some(allowed_email.to_string()),
                false,
            )),
        );
    }
}
