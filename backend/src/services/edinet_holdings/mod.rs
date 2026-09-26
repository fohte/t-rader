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
        db: &impl sea_orm::ConnectionTrait,
    ) -> Result<Option<NaiveDate>, sea_orm::DbErr>;

    async fn fetch(
        source: &dyn ShareholdingStructureSource,
        date: NaiveDate,
    ) -> Result<Vec<Self::Document>, ShareholdingStructureSourceError>;

    async fn upsert(
        db: &impl sea_orm::ConnectionTrait,
        documents: Vec<Self::Document>,
    ) -> Result<usize, sea_orm::DbErr>;
}

enum IngestDateError {
    Source(ShareholdingStructureSourceError),
    Database(sea_orm::DbErr),
}

async fn ingest_date<T: EdinetEndpoint>(
    db: &impl sea_orm::ConnectionTrait,
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
    db: &impl sea_orm::ConnectionTrait,
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
    db: &impl sea_orm::ConnectionTrait,
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
    db: &impl sea_orm::ConnectionTrait,
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

async fn run_all(db: &impl sea_orm::ConnectionTrait, source: &dyn ShareholdingStructureSource) {
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
    use chrono::{DateTime, NaiveDate, Utc};
    use sea_orm::EntityTrait;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, ResponseTemplate};

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::entities::{
        cross_shareholding_documents, large_volume_shareholding_documents,
        major_shareholder_documents,
    };
    use crate::models::jquants_plan::JQuantsPlan;
    use core_domain::holdings::{
        CrossShareholding, CrossShareholdingCategory, CrossShareholdingContent,
        CrossShareholdingDocument, LargeVolumeReportType, LargeVolumeShareholdingContent,
        LargeVolumeShareholdingDocument, MajorShareholder, MajorShareholderContent,
        MajorShareholderDocument, MajorShareholderReportType, MutualHolding,
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
        stored_document_with_change_reason(document_id, stock_code, submitted_on, None)
    }

    fn stored_document_with_change_reason(
        document_id: &str,
        stock_code: &str,
        submitted_on: NaiveDate,
        change_reason: Option<&str>,
    ) -> LargeVolumeShareholdingDocument {
        LargeVolumeShareholdingDocument {
            metadata: ShareholdingDocumentMetadata {
                document_id: document_id.to_string(),
                stock_code: Some(stock_code.to_string()),
                filer_code: "E99999".to_string(),
                submitted_on,
            },
            content: Some(LargeVolumeShareholdingContent {
                report_type: LargeVolumeReportType::Unknown,
                change_reason: change_reason.map(str::to_string),
                total_shares_ratio: None,
                previous_total_shares_ratio: None,
                holders: vec![],
            }),
        }
    }

    fn plan_refetch_window(plan: JQuantsPlan) -> (NaiveDate, NaiveDate) {
        let (_, to) = plan.range(Utc::now().date_naive());
        (to - ChronoDuration::days(REFETCH_WINDOW_DAYS), to)
    }

    async fn seed_latest_large_volume_document(
        db: &impl sea_orm::ConnectionTrait,
        document_id: &str,
        submitted_on: NaiveDate,
    ) {
        large_volume_shareholdings::Endpoint::upsert(
            db,
            vec![stored_document(document_id, "99990", submitted_on)],
        )
        .await
        .expect("seed document");
    }

    async fn mock_large_volume_range(
        mock: &JQuantsMockServer,
        from: NaiveDate,
        to: NaiveDate,
        mut documents_for: impl FnMut(NaiveDate) -> Vec<serde_json::Value>,
    ) {
        let mut date = from;
        while date <= to {
            mock.edinet_documents("/edinet/large-volume-shareholders")
                .date(&date.format("%Y%m%d").to_string())
                .docs(documents_for(date))
                .ok()
                .await;
            date += ChronoDuration::days(1);
        }
    }

    fn stable_timestamp() -> DateTime<chrono::FixedOffset> {
        DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
    }

    fn normalize_large_volume_timestamps(
        mut model: large_volume_shareholding_documents::Model,
    ) -> large_volume_shareholding_documents::Model {
        model.created_at = stable_timestamp();
        model.updated_at = stable_timestamp();
        model
    }

    fn normalize_major_shareholder_timestamps(
        mut model: major_shareholder_documents::Model,
    ) -> major_shareholder_documents::Model {
        model.created_at = stable_timestamp();
        model.updated_at = stable_timestamp();
        model
    }

    fn normalize_cross_shareholding_timestamps(
        mut model: cross_shareholding_documents::Model,
    ) -> cross_shareholding_documents::Model {
        model.created_at = stable_timestamp();
        model.updated_at = stable_timestamp();
        model
    }

    async fn find_all(
        db: &impl sea_orm::ConnectionTrait,
    ) -> Vec<large_volume_shareholding_documents::Model> {
        large_volume_shareholding_documents::Entity::find()
            .all(db)
            .await
            .expect("find documents")
    }

    #[backend_test_macros::database_test]
    async fn fetches_within_the_configured_plan_range(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");
        let (from, to) = plan_refetch_window(JQuantsPlan::Standard);
        seed_latest_large_volume_document(&db, "SAMPLE-DOC-0", to).await;
        mock_large_volume_range(&mock, from, to, |date| {
            if date == from {
                vec![document("SAMPLE-DOC-0", "99990", date)]
            } else if date == from + ChronoDuration::days(1) {
                vec![document("SAMPLE-DOC-1", "99990", date)]
            } else if date == from + ChronoDuration::days(2) {
                vec![document("SAMPLE-DOC-2", "99990", date)]
            } else {
                vec![]
            }
        })
        .await;
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        let mut rows = find_all(&db)
            .await
            .into_iter()
            .map(normalize_large_volume_timestamps)
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.document_id.cmp(&right.document_id));

        assert_eq!(
            (stats, rows),
            (
                IngestStats {
                    days_processed: REFETCH_WINDOW_DAYS as usize + 1,
                    documents_saved: 3,
                    failed_dates: 0,
                },
                vec![
                    large_volume_shareholding_documents::Model {
                        document_id: "SAMPLE-DOC-0".to_string(),
                        stock_code: Some("99990".to_string()),
                        filer_code: "E99999".to_string(),
                        submitted_on: from,
                        details: json!({
                            "report_type": "unknown",
                            "change_reason": null,
                            "total_shares_ratio": null,
                            "previous_total_shares_ratio": null,
                            "holders": [],
                        }),
                        created_at: stable_timestamp(),
                        updated_at: stable_timestamp(),
                    },
                    large_volume_shareholding_documents::Model {
                        document_id: "SAMPLE-DOC-1".to_string(),
                        stock_code: Some("99990".to_string()),
                        filer_code: "E99999".to_string(),
                        submitted_on: from + ChronoDuration::days(1),
                        details: json!({
                            "report_type": "unknown",
                            "change_reason": null,
                            "total_shares_ratio": null,
                            "previous_total_shares_ratio": null,
                            "holders": [],
                        }),
                        created_at: stable_timestamp(),
                        updated_at: stable_timestamp(),
                    },
                    large_volume_shareholding_documents::Model {
                        document_id: "SAMPLE-DOC-2".to_string(),
                        stock_code: Some("99990".to_string()),
                        filer_code: "E99999".to_string(),
                        submitted_on: from + ChronoDuration::days(2),
                        details: json!({
                            "report_type": "unknown",
                            "change_reason": null,
                            "total_shares_ratio": null,
                            "previous_total_shares_ratio": null,
                            "holders": [],
                        }),
                        created_at: stable_timestamp(),
                        updated_at: stable_timestamp(),
                    },
                ],
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn refetches_from_latest_submitted_on_minus_window(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");
        let (_, range_to) = JQuantsPlan::Standard.range(Utc::now().date_naive());
        let latest = range_to - ChronoDuration::days(2);
        let expected_start = latest - ChronoDuration::days(REFETCH_WINDOW_DAYS);
        let to = range_to;

        seed_latest_large_volume_document(&db, "SAMPLE-DOC", latest).await;
        mock_large_volume_range(&mock, expected_start, to, |_| vec![]).await;
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

    #[backend_test_macros::database_test]
    async fn upserts_the_document_with_the_same_id(db: crate::database::DatabaseHandle) {
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date");
        let saved = large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![stored_document_with_change_reason(
                "SAMPLE-DOC",
                "99990",
                date,
                Some("initial"),
            )],
        )
        .await
        .expect("first upsert");
        let second_saved = large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![stored_document_with_change_reason(
                "SAMPLE-DOC",
                "88880",
                date,
                Some("corrected"),
            )],
        )
        .await
        .expect("second upsert");

        let rows = find_all(&db)
            .await
            .into_iter()
            .map(normalize_large_volume_timestamps)
            .collect::<Vec<_>>();
        assert_eq!(
            (saved, second_saved, rows),
            (
                1,
                1,
                vec![large_volume_shareholding_documents::Model {
                    document_id: "SAMPLE-DOC".to_string(),
                    stock_code: Some("88880".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on: date,
                    details: json!({
                        "report_type": "unknown",
                        "change_reason": "corrected",
                        "total_shares_ratio": null,
                        "previous_total_shares_ratio": null,
                        "holders": [],
                    }),
                    created_at: stable_timestamp(),
                    updated_at: stable_timestamp(),
                }]
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn upserts_major_shareholder_documents(db: crate::database::DatabaseHandle) {
        let submitted_on = NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date");
        let period_end = NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date");
        let saved = major_shareholders::Endpoint::upsert(
            &db,
            vec![MajorShareholderDocument {
                metadata: ShareholdingDocumentMetadata {
                    document_id: "SAMPLE-MAJOR-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on,
                },
                content: Some(MajorShareholderContent {
                    period_end: Some(period_end),
                    report_type: MajorShareholderReportType::Annual,
                    holders: vec![MajorShareholder {
                        rank: Some(1),
                        name: "Example Holder".to_string(),
                        shares_held: Some(1200),
                        shares_ratio: Some(0.12),
                    }],
                }),
            }],
        )
        .await
        .expect("upsert major shareholders");
        let rows = major_shareholder_documents::Entity::find()
            .all(&db)
            .await
            .expect("find major shareholder documents")
            .into_iter()
            .map(normalize_major_shareholder_timestamps)
            .collect::<Vec<_>>();

        assert_eq!(
            (saved, rows),
            (
                1,
                vec![major_shareholder_documents::Model {
                    document_id: "SAMPLE-MAJOR-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on,
                    details: json!({
                        "period_end": "2024-12-31",
                        "report_type": "annual",
                        "holders": [{
                            "rank": 1,
                            "name": "Example Holder",
                            "shares_held": 1200,
                            "shares_ratio": 0.12,
                        }],
                    }),
                    created_at: stable_timestamp(),
                    updated_at: stable_timestamp(),
                }]
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn upserts_cross_shareholding_documents(db: crate::database::DatabaseHandle) {
        let submitted_on = NaiveDate::from_ymd_opt(2025, 1, 6).expect("valid date");
        let period_end = NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date");
        let saved = cross_shareholdings::Endpoint::upsert(
            &db,
            vec![CrossShareholdingDocument {
                metadata: ShareholdingDocumentMetadata {
                    document_id: "SAMPLE-CROSS-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on,
                },
                content: Some(CrossShareholdingContent {
                    period_end: Some(period_end),
                    holdings: vec![CrossShareholding {
                        issuer_name: "Example Issuer".to_string(),
                        issuer_stock_code: Some("88880".to_string()),
                        category: CrossShareholdingCategory::Specified,
                        current_shares: Some(500),
                        previous_shares: Some(400),
                        current_book_value: Some(6000),
                        previous_book_value: Some(5000),
                        mutual_holding: MutualHolding::Held,
                    }],
                }),
            }],
        )
        .await
        .expect("upsert cross shareholdings");
        let rows = cross_shareholding_documents::Entity::find()
            .all(&db)
            .await
            .expect("find cross shareholding documents")
            .into_iter()
            .map(normalize_cross_shareholding_timestamps)
            .collect::<Vec<_>>();

        assert_eq!(
            (saved, rows),
            (
                1,
                vec![cross_shareholding_documents::Model {
                    document_id: "SAMPLE-CROSS-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on,
                    details: json!({
                        "period_end": "2024-12-31",
                        "holdings": [{
                            "issuer_name": "Example Issuer",
                            "issuer_stock_code": "88880",
                            "category": "specified",
                            "current_shares": 500,
                            "previous_shares": 400,
                            "current_book_value": 6000,
                            "previous_book_value": 5000,
                            "mutual_holding": "held",
                        }],
                    }),
                    created_at: stable_timestamp(),
                    updated_at: stable_timestamp(),
                }]
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn retries_a_failed_date_within_the_same_cycle_and_recovers(
        db: crate::database::DatabaseHandle,
    ) {
        let mock = JQuantsMockServer::start().await;
        let (from, to) = plan_refetch_window(JQuantsPlan::Standard);
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");
        seed_latest_large_volume_document(&db, "SAMPLE-DOC", to).await;

        Mock::given(method("GET"))
            .and(path("/edinet/large-volume-shareholders"))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .up_to_n_times(1)
            .mount(mock.server_ref())
            .await;
        mock_large_volume_range(&mock, from, to, |date| {
            if date == from {
                vec![document("SAMPLE-DOC", "99990", date)]
            } else {
                vec![]
            }
        })
        .await;
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        let rows = find_all(&db)
            .await
            .into_iter()
            .map(normalize_large_volume_timestamps)
            .collect::<Vec<_>>();
        assert_eq!(
            (stats, rows),
            (
                IngestStats {
                    days_processed: REFETCH_WINDOW_DAYS as usize + 1,
                    documents_saved: 1,
                    failed_dates: 0,
                },
                vec![large_volume_shareholding_documents::Model {
                    document_id: "SAMPLE-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on: from,
                    details: json!({
                        "report_type": "unknown",
                        "change_reason": null,
                        "total_shares_ratio": null,
                        "previous_total_shares_ratio": null,
                        "holders": [],
                    }),
                    created_at: stable_timestamp(),
                    updated_at: stable_timestamp(),
                }],
            )
        );
    }

    #[backend_test_macros::database_test]
    async fn counts_a_date_when_its_retry_fails(db: crate::database::DatabaseHandle) {
        let mock = JQuantsMockServer::start().await;
        let (from, to) = plan_refetch_window(JQuantsPlan::Standard);
        let client = mock
            .client_with_plan(JQuantsPlan::Standard)
            .expect("client");
        seed_latest_large_volume_document(&db, "SAMPLE-DOC", to).await;

        Mock::given(method("GET"))
            .and(path("/edinet/large-volume-shareholders"))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .mount(mock.server_ref())
            .await;
        mock_large_volume_range(&mock, from + ChronoDuration::days(1), to, |_| vec![]).await;
        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle succeeds");

        let rows = find_all(&db)
            .await
            .into_iter()
            .map(normalize_large_volume_timestamps)
            .collect::<Vec<_>>();
        assert_eq!(
            (stats, rows),
            (
                IngestStats {
                    days_processed: REFETCH_WINDOW_DAYS as usize + 1,
                    documents_saved: 0,
                    failed_dates: 1,
                },
                vec![large_volume_shareholding_documents::Model {
                    document_id: "SAMPLE-DOC".to_string(),
                    stock_code: Some("99990".to_string()),
                    filer_code: "E99999".to_string(),
                    submitted_on: to,
                    details: json!({
                        "report_type": "unknown",
                        "change_reason": null,
                        "total_shares_ratio": null,
                        "previous_total_shares_ratio": null,
                        "holders": [],
                    }),
                    created_at: stable_timestamp(),
                    updated_at: stable_timestamp(),
                }],
            )
        );
    }
}
