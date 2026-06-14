//! MCP session の PostgreSQL ベース永続化
//!
//! `rmcp` の [`SessionStore`] trait を実装し、`initialize` ハンドシェイクのパラメータを
//! `mcp_session_state` テーブルに保存する。バックエンドの再起動を跨いだ session ID
//! でリクエストが来た際、`StreamableHttpService` はこの store から state を読み出して
//! `LocalSessionManager` に新規 in-memory session を確保させ、initialize を replay する。

use std::time::Duration;

use chrono::Utc;
use rmcp::transport::streamable_http_server::session::store::{
    SessionState, SessionStore, SessionStoreError,
};
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict,
};

use crate::entities::mcp_session_state;

pub const DEFAULT_SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub const DEFAULT_GC_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub struct PostgresSessionStore {
    db: DatabaseConnection,
    ttl: Duration,
}

impl PostgresSessionStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            ttl: DEFAULT_SESSION_TTL,
        }
    }

    #[cfg(test)]
    pub fn with_ttl(db: DatabaseConnection, ttl: Duration) -> Self {
        Self { db, ttl }
    }

    /// 保持期間を超えた session 行を削除する。
    pub async fn purge_expired(&self) -> Result<u64, sea_orm::DbErr> {
        let result = mcp_session_state::Entity::delete_many()
            .filter(mcp_session_state::Column::UpdatedAt.lt(self.expiration_threshold()))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    fn expiration_threshold(&self) -> chrono::DateTime<Utc> {
        let ttl_secs = i64::try_from(self.ttl.as_secs()).unwrap_or(i64::MAX);
        Utc::now() - chrono::Duration::seconds(ttl_secs)
    }
}

#[async_trait::async_trait]
impl SessionStore for PostgresSessionStore {
    async fn load(&self, session_id: &str) -> Result<Option<SessionState>, SessionStoreError> {
        let row = mcp_session_state::Entity::find_by_id(session_id.to_owned())
            .one(&self.db)
            .await
            .map_err(into_store_error)?;

        let Some(row) = row else {
            return Ok(None);
        };

        if row.updated_at < self.expiration_threshold() {
            // inline GC は best-effort: 失敗しても session 解決 (Ok(None) で未知扱い) は継続させ、
            // 後段の StreamableHttpService が新規 initialize を案内できるようにする。
            if let Err(err) = mcp_session_state::Entity::delete_by_id(session_id.to_owned())
                .exec(&self.db)
                .await
            {
                tracing::warn!(
                    error = %err,
                    session_id,
                    "failed to delete expired mcp session inline"
                );
            }
            return Ok(None);
        }

        let state: SessionState = serde_json::from_value(row.state).map_err(into_store_error)?;
        Ok(Some(state))
    }

    async fn store(&self, session_id: &str, state: &SessionState) -> Result<(), SessionStoreError> {
        let payload = serde_json::to_value(state).map_err(into_store_error)?;
        let now = Utc::now().fixed_offset();
        let model = mcp_session_state::ActiveModel {
            session_id: ActiveValue::set(session_id.to_owned()),
            state: ActiveValue::set(payload),
            created_at: ActiveValue::set(now),
            updated_at: ActiveValue::set(now),
        };

        mcp_session_state::Entity::insert(model)
            .on_conflict(
                OnConflict::column(mcp_session_state::Column::SessionId)
                    .update_columns([
                        mcp_session_state::Column::State,
                        mcp_session_state::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(into_store_error)?;

        Ok(())
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionStoreError> {
        mcp_session_state::Entity::delete_by_id(session_id.to_owned())
            .exec(&self.db)
            .await
            .map_err(into_store_error)?;
        Ok(())
    }
}

fn into_store_error<E>(err: E) -> SessionStoreError
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(err)
}

/// 期限切れ session を定期的に削除するバックグラウンドタスクを起動する。
pub fn spawn_gc(store: PostgresSessionStore, interval: Duration) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // 起動直後の即時実行を避ける。
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match store.purge_expired().await {
                Ok(removed) if removed > 0 => {
                    tracing::info!(removed, "purged expired mcp sessions");
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "failed to purge expired mcp sessions");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use rmcp::model::{ClientCapabilities, Implementation, InitializeRequestParams};
    use sea_orm::{Database, DatabaseConnection};

    use super::*;

    /// テスト用 DB 接続。
    ///
    /// `TEST_DATABASE_URL` が設定されていない場合は `None` を返してテストを skip する。
    /// (CI / ローカルどちらでも、戦略的に DB ありテストを走らせる場合のみ実行する)
    async fn maybe_db() -> Option<DatabaseConnection> {
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        Database::connect(&url).await.ok()
    }

    fn sample_state(client_name: &str) -> SessionState {
        SessionState::new(InitializeRequestParams::new(
            ClientCapabilities::default(),
            Implementation::new(client_name.to_string(), "0.0.0".to_string()),
        ))
    }

    #[tokio::test]
    async fn store_load_delete_roundtrip() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };
        let store = PostgresSessionStore::new(db);
        let session_id = format!("test-{}", uuid::Uuid::new_v4());
        let state = sample_state("roundtrip-client");

        store.store(&session_id, &state).await.unwrap();

        let loaded = store.load(&session_id).await.unwrap().unwrap();
        assert_eq!(
            loaded.initialize_params.client_info.name,
            "roundtrip-client"
        );

        store.delete(&session_id).await.unwrap();
        assert!(store.load(&session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn survives_simulated_restart() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };

        let session_id = format!("restart-{}", uuid::Uuid::new_v4());
        let state = sample_state("restart-client");

        {
            // 1 つ目の "プロセス" として store に書き込む。
            let store = PostgresSessionStore::new(db.clone());
            store.store(&session_id, &state).await.unwrap();
        }

        {
            // 別の store インスタンス = 再起動後のプロセスを想定。
            // in-memory state を持たないが、DB から initialize_params を引ける。
            let store = PostgresSessionStore::new(db.clone());
            let loaded = store.load(&session_id).await.unwrap();
            assert_eq!(
                loaded
                    .map(|s| s.initialize_params.client_info.name)
                    .as_deref(),
                Some("restart-client")
            );
            store.delete(&session_id).await.unwrap();
        }
    }

    #[tokio::test]
    async fn purges_expired_rows() {
        let Some(db) = maybe_db().await else {
            eprintln!("TEST_DATABASE_URL not set; skipping");
            return;
        };

        let store = PostgresSessionStore::with_ttl(db, Duration::from_secs(0));
        let session_id = format!("gc-{}", uuid::Uuid::new_v4());
        store
            .store(&session_id, &sample_state("gc-client"))
            .await
            .unwrap();

        // 即時 TTL なので load は None を返し、行も削除されている。
        assert!(store.load(&session_id).await.unwrap().is_none());
        store.purge_expired().await.unwrap();
        assert!(store.load(&session_id).await.unwrap().is_none());
    }
}
