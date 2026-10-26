use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use core_application::{
    tavily_retrieval_cache::{
        TavilyCredentialGeneration, TavilyRetrievalCacheEntry, TavilyRetrievalCacheKey,
        TavilyRetrievalCacheLookup, TavilyRetrievalCacheRepository, TavilyRetrievalCacheWrite,
    },
    unit_of_work::SharedUnitOfWork,
    web_search::{WebSearchTimeRange, WebSearchTopic},
};
use sea_orm::{ConnectionTrait, DatabaseBackend, SqlxPostgresConnector, Statement};
use serde_json::json;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::PostgresTavilyRetrievalCacheRepository;
use crate::{DatabaseHandle, PostgresUnitOfWork};

fn key() -> TavilyRetrievalCacheKey {
    TavilyRetrievalCacheKey::search(
        "synthetic-scope",
        "synthetic-account",
        TavilyCredentialGeneration::from_secret(b"synthetic-key"),
        &format!("synthetic query {}", Uuid::new_v4()),
        Some(WebSearchTopic::General),
        Some(WebSearchTimeRange::Week),
    )
}

fn timestamp() -> DateTime<Utc> {
    DateTime::from_timestamp(1_767_225_600, 0).expect("fixed test timestamp")
}

async fn test_database() -> DatabaseHandle {
    let pool = crate::test_support::create_test_pool_with_max_connections(1).await;
    DatabaseHandle::from(SqlxPostgresConnector::from_sqlx_postgres_pool(pool))
}

fn repository(db: DatabaseHandle) -> PostgresTavilyRetrievalCacheRepository {
    let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(db));
    PostgresTavilyRetrievalCacheRepository::new(unit_of_work)
}

#[backend_test_macros::database_test]
async fn expires_cache_entries_at_the_expiry_timestamp(_db: DatabaseHandle) {
    let db = test_database().await;
    let repository = repository(db);
    let key = key();
    let fetched_at = timestamp();
    let expires_at = fetched_at + Duration::minutes(15);
    let payload = json!([{"title": "Synthetic result"}]);
    let lookup = repository
        .acquire(key.clone(), fetched_at)
        .await
        .expect("acquire cache key");
    let lease = match lookup {
        TavilyRetrievalCacheLookup::Miss(lease) => lease,
        TavilyRetrievalCacheLookup::Hit(_) => panic!("fresh cache is empty"),
    };
    lease
        .store(TavilyRetrievalCacheWrite {
            payload: payload.clone(),
            fetched_at,
            expires_at,
        })
        .await
        .expect("store cache entry");

    let lookup = repository
        .acquire(key.clone(), expires_at - Duration::milliseconds(1))
        .await
        .expect("acquire valid cache entry");
    let entry = match lookup {
        TavilyRetrievalCacheLookup::Hit(entry) => entry,
        TavilyRetrievalCacheLookup::Miss(_) => panic!("cache entry remains valid before expiry"),
    };
    assert_eq!(
        entry,
        TavilyRetrievalCacheEntry {
            payload,
            fetched_at,
        },
    );
    let lookup = repository
        .acquire(key, expires_at)
        .await
        .expect("acquire expired cache key");
    let is_miss = matches!(lookup, TavilyRetrievalCacheLookup::Miss(_));
    assert!(is_miss);
    if let TavilyRetrievalCacheLookup::Miss(lease) = lookup {
        lease.release().await.expect("release cache lease");
    }
}

#[backend_test_macros::database_test]
async fn failed_cache_store_releases_its_claim(_db: DatabaseHandle) {
    let repository = repository(test_database().await);
    let key = key();
    let fetched_at = timestamp();
    let lookup = repository
        .acquire(key.clone(), fetched_at)
        .await
        .expect("acquire cache key");
    let lease = match lookup {
        TavilyRetrievalCacheLookup::Miss(lease) => lease,
        TavilyRetrievalCacheLookup::Hit(_) => panic!("fresh cache is empty"),
    };
    let store_result = lease
        .store(TavilyRetrievalCacheWrite {
            payload: json!({"content": "Synthetic page"}),
            fetched_at,
            expires_at: fetched_at,
        })
        .await;
    let retry = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        repository.acquire(key, fetched_at),
    )
    .await
    .expect("released claim can be acquired promptly")
    .expect("acquire cache key after failed store");
    let retry_is_miss = matches!(&retry, TavilyRetrievalCacheLookup::Miss(_));

    assert_eq!((store_result.is_err(), retry_is_miss), (true, true));
    if let TavilyRetrievalCacheLookup::Miss(lease) = retry {
        lease.release().await.expect("release retry lease");
    }
}

#[backend_test_macros::database_test]
async fn concurrent_misses_share_the_first_stored_result(_db: DatabaseHandle) {
    let db = test_database().await;
    let repository = repository(db.clone());
    let key = key();
    let fetched_at = timestamp();
    let first = repository
        .acquire(key.clone(), fetched_at)
        .await
        .expect("acquire first cache key");
    let first_lease = match first {
        TavilyRetrievalCacheLookup::Miss(lease) => lease,
        TavilyRetrievalCacheLookup::Hit(_) => panic!("fresh cache is empty"),
    };

    let second_repository = repository.clone();
    let second_key = key.clone();
    let (started_sender, started_receiver) = oneshot::channel();
    let mut second = tokio::spawn(async move {
        let _ = started_sender.send(());
        second_repository.acquire(second_key, fetched_at).await
    });
    started_receiver.await.expect("second cache lookup started");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut second)
            .await
            .is_err(),
    );
    first_lease
        .store(TavilyRetrievalCacheWrite {
            payload: json!({"content": "Synthetic page"}),
            fetched_at,
            expires_at: fetched_at + Duration::hours(24),
        })
        .await
        .expect("store first result");

    let second = second
        .await
        .expect("complete second cache lookup")
        .expect("acquire second cache key");
    let entry = match second {
        TavilyRetrievalCacheLookup::Hit(entry) => entry,
        TavilyRetrievalCacheLookup::Miss(_) => panic!("concurrent lookup reuses stored result"),
    };
    assert_eq!(
        entry,
        TavilyRetrievalCacheEntry {
            payload: json!({"content": "Synthetic page"}),
            fetched_at,
        },
    );
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "DELETE FROM tavily_retrieval_cache WHERE cache_key = $1",
        [key.as_bytes().to_vec().into()],
    ))
    .await
    .expect("clean up cache entry");
}
