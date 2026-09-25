//! 保有構造の書類を取得元 port 経由で定期的に取り込み、DB に蓄積する。

mod cross_shareholdings;
mod large_volume_shareholdings;
mod major_shareholders;

use std::time::Duration;

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use core_application::{
    SharedShareholdingStructureSource, ShareholdingStructureSource,
    ShareholdingStructureSourceError,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryOrder};
use serde::Serialize;
use tokio::task::JoinHandle;

/// ポーリング実行間隔。日次で更新されるデータに対して 1 日間隔とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 既存データの最新日からこの日数分遡って再取得する。
const REFETCH_WINDOW_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_processed: usize,
    pub documents_saved: usize,
    pub failed_dates: usize,
}

#[async_trait]
trait EdinetEndpoint: Send + Sync {
    type Document: Send;

    const NAME: &'static str;

    fn available_from() -> NaiveDate;

    async fn latest_submitted_on(
        db: &DatabaseConnection,
    ) -> Result<Option<NaiveDate>, sea_orm::DbErr>;

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError>;

    async fn upsert(
        db: &DatabaseConnection,
        documents: Vec<Self::Document>,
    ) -> Result<usize, sea_orm::DbErr>;
}

enum IngestDateError {
    Source(ShareholdingStructureSourceError),
    Database(sea_orm::DbErr),
}

async fn ingest_date<T: EdinetEndpoint>(
    db: &DatabaseConnection,
    source: &dyn ShareholdingStructureSource,
    date: NaiveDate,
) -> Result<usize, IngestDateError> {
    let documents = T::fetch(source, date)
        .await
        .map_err(IngestDateError::Source)?;
    T::upsert(db, documents)
        .await
        .map_err(IngestDateError::Database)
}

async fn run_ingest_cycle<T: EdinetEndpoint>(
    db: &DatabaseConnection,
    source: &dyn ShareholdingStructureSource,
) -> Result<IngestStats, sea_orm::DbErr> {
    let Some(fetchable_range) = source.fetchable_range(Utc::now().date_naive()) else {
        tracing::info!(
            endpoint = T::NAME,
            "取得可能範囲が未検出のため取り込みをスキップします"
        );
        return Ok(IngestStats::default());
    };

    let earliest = fetchable_range.from.max(T::available_from());
    let start = match T::latest_submitted_on(db).await? {
        Some(latest) => (latest - ChronoDuration::days(REFETCH_WINDOW_DAYS)).max(earliest),
        None => earliest,
    };

    let mut stats = IngestStats::default();
    let mut retry_dates = Vec::new();
    let mut date = start;
    while date <= fetchable_range.to {
        match ingest_date::<T>(db, source, date).await {
            Ok(count) => stats.documents_saved += count,
            Err(IngestDateError::Source(error)) => {
                tracing::warn!(endpoint = T::NAME, %date, %error, "書類の取得に失敗し、この cycle 終了後に再試行します");
                retry_dates.push(date);
            }
            Err(IngestDateError::Database(error)) => return Err(error),
        }
        stats.days_processed += 1;
        date += ChronoDuration::days(1);
    }

    for date in retry_dates {
        match ingest_date::<T>(db, source, date).await {
            Ok(count) => stats.documents_saved += count,
            Err(IngestDateError::Source(error)) => {
                stats.failed_dates += 1;
                tracing::error!(
                    endpoint = T::NAME,
                    %date,
                    %error,
                    "書類の再取得にも失敗しました。この日付の書類は取り込めていません"
                );
            }
            Err(IngestDateError::Database(error)) => return Err(error),
        }
    }

    Ok(stats)
}

async fn latest_submitted_on_of<E, C>(
    db: &DatabaseConnection,
    submitted_on_column: C,
    submitted_on: impl Fn(&E::Model) -> NaiveDate,
) -> Result<Option<NaiveDate>, sea_orm::DbErr>
where
    E: EntityTrait,
    C: ColumnTrait,
{
    let latest = E::find().order_by_desc(submitted_on_column).one(db).await?;
    Ok(latest.map(|model| submitted_on(&model)))
}

async fn upsert_documents<E, T>(
    db: &DatabaseConnection,
    documents: Vec<T>,
    build: impl Fn(T) -> Result<E::ActiveModel, sea_orm::DbErr>,
    conflict: sea_orm::sea_query::OnConflict,
) -> Result<usize, sea_orm::DbErr>
where
    E: EntityTrait,
    E::ActiveModel: Send,
{
    let count = documents.len();
    if count == 0 {
        return Ok(0);
    }

    let models = documents
        .into_iter()
        .map(build)
        .collect::<Result<Vec<_>, _>>()?;
    E::insert_many(models)
        .on_conflict(conflict)
        .exec_without_returning(db)
        .await?;

    Ok(count)
}

fn serialize_details<T: Serialize>(details: T) -> Result<serde_json::Value, sea_orm::DbErr> {
    serde_json::to_value(details).map_err(|error| sea_orm::DbErr::Custom(error.to_string()))
}

/// poll task を起動する。初回は即実行し、その後 `interval` で繰り返す。
pub fn spawn_poll(
    db: DatabaseConnection,
    source: SharedShareholdingStructureSource,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            run_all(&db, source.as_ref()).await;
        }
    })
}

async fn run_all(db: &DatabaseConnection, source: &dyn ShareholdingStructureSource) {
    let results = [
        (
            large_volume_shareholdings::Endpoint::NAME,
            run_ingest_cycle::<large_volume_shareholdings::Endpoint>(db, source).await,
        ),
        (
            cross_shareholdings::Endpoint::NAME,
            run_ingest_cycle::<cross_shareholdings::Endpoint>(db, source).await,
        ),
        (
            major_shareholders::Endpoint::NAME,
            run_ingest_cycle::<major_shareholders::Endpoint>(db, source).await,
        ),
    ];
    for (name, result) in results {
        match result {
            Ok(stats) => {
                tracing::debug!(endpoint = name, ?stats, "保有構造の取り込みが完了しました")
            }
            Err(error) => {
                tracing::warn!(endpoint = name, %error, "保有構造の取り込みに失敗しました")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::EntityTrait;
    use serde_json::json;
    use sqlx::PgPool;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, ResponseTemplate};

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::entities::large_volume_shareholding_documents;
    use crate::testing::create_test_db;
    use core_domain::holdings::{
        LargeVolumeReportType, LargeVolumeShareholdingContent, LargeVolumeShareholdingDocument,
        ShareholdingDocumentMetadata,
    };

    fn document(document_id: &str, stock_code: &str, date: NaiveDate) -> serde_json::Value {
        json!({
            "DocId": document_id,
            "Code": stock_code,
            "EdinetCode": "E99999",
            "SubDate": date.format("%Y-%m-%d").to_string(),
        })
    }

    fn stored_document(
        document_id: &str,
        stock_code: &str,
        submitted_on: NaiveDate,
    ) -> LargeVolumeShareholdingDocument {
        LargeVolumeShareholdingDocument {
            metadata: ShareholdingDocumentMetadata {
                document_id: document_id.to_string(),
                stock_code: Some(stock_code.to_string()),
                filer_code: "E99999".to_string(),
                submitted_on,
            },
            content: LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Unknown,
                change_reason: None,
                total_shares_ratio: None,
                previous_total_shares_ratio: None,
                holders: vec![],
            },
        }
    }

    async fn find_all(db: &DatabaseConnection) -> Vec<large_volume_shareholding_documents::Model> {
        large_volume_shareholding_documents::Entity::find()
            .all(db)
            .await
            .expect("find documents")
    }

    #[sqlx::test(migrations = false)]
    async fn fetches_from_available_from_when_table_is_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(2);

        for offset in 0..=2 {
            let date = from + ChronoDuration::days(offset);
            mock.edinet_documents("/edinet/large-volume-shareholders")
                .date(&date.format("%Y%m%d").to_string())
                .docs(vec![document(
                    &format!("SAMPLE-DOC-{offset}"),
                    "99990",
                    date,
                )])
                .ok()
                .await;
        }

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (stats, find_all(&db).await.len()),
            (
                IngestStats {
                    days_processed: 3,
                    documents_saved: 3,
                    failed_dates: 0,
                },
                3,
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn refetches_from_latest_submitted_on_minus_window(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let from = large_volume_shareholdings::Endpoint::available_from();
        let latest = from + ChronoDuration::days(60);
        let expected_start = latest - ChronoDuration::days(REFETCH_WINDOW_DAYS);
        let to = expected_start + ChronoDuration::days(1);

        large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![stored_document("SAMPLE-DOC", "99990", latest)],
        )
        .await
        .expect("seed document");

        let mut date = expected_start;
        while date <= to {
            mock.edinet_documents("/edinet/large-volume-shareholders")
                .date(&date.format("%Y%m%d").to_string())
                .docs(vec![])
                .ok()
                .await;
            date += ChronoDuration::days(1);
        }

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");
        let expected_days = (to - expected_start).num_days() as usize + 1;

        assert_eq!(
            stats,
            IngestStats {
                days_processed: expected_days,
                documents_saved: 0,
                failed_dates: 0,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn skips_when_fetchable_range_is_unknown(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (stats, find_all(&db).await.len()),
            (IngestStats::default(), 0)
        );
    }

    #[sqlx::test(migrations = false)]
    async fn upserts_the_document_with_the_same_id(pool: PgPool) {
        let db = create_test_db(pool).await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date");
        let saved = large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![stored_document("SAMPLE-DOC", "99990", date)],
        )
        .await
        .expect("first upsert");
        let second_saved = large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![stored_document("SAMPLE-DOC", "88880", date)],
        )
        .await
        .expect("second upsert");

        let rows = find_all(&db).await;
        assert_eq!(
            (
                saved,
                second_saved,
                rows.len(),
                rows[0].stock_code.as_deref()
            ),
            (1, 1, 1, Some("88880"))
        );
    }

    #[sqlx::test(migrations = false)]
    async fn retries_a_failed_date_within_the_same_cycle_and_recovers(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(1);

        Mock::given(method("GET"))
            .and(path("/edinet/large-volume-shareholders"))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .up_to_n_times(1)
            .mount(mock.server_ref())
            .await;
        mock.edinet_documents("/edinet/large-volume-shareholders")
            .date(&from.format("%Y%m%d").to_string())
            .docs(vec![document("SAMPLE-DOC", "99990", from)])
            .ok()
            .await;
        mock.edinet_documents("/edinet/large-volume-shareholders")
            .date(&to.format("%Y%m%d").to_string())
            .docs(vec![])
            .ok()
            .await;

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (stats, find_all(&db).await.len()),
            (
                IngestStats {
                    days_processed: 2,
                    documents_saved: 1,
                    failed_dates: 0,
                },
                1,
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn counts_a_date_when_its_retry_fails(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(1);

        Mock::given(method("GET"))
            .and(path("/edinet/large-volume-shareholders"))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .mount(mock.server_ref())
            .await;
        mock.edinet_documents("/edinet/large-volume-shareholders")
            .date(&to.format("%Y%m%d").to_string())
            .docs(vec![])
            .ok()
            .await;

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (stats, find_all(&db).await.len()),
            (
                IngestStats {
                    days_processed: 2,
                    documents_saved: 0,
                    failed_dates: 1,
                },
                0,
            )
        );
    }
}
