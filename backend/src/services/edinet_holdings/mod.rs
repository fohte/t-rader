//! J-Quants EDINET 由来のデータ (大量保有報告書 / 政策保有株式 / 大株主状況) を
//! 定期的に取り込む。書類の訂正は同じ DocId への上書きとして反映されるため、
//! 差分取得の仕組みは無く、提出日を範囲指定して再取得することで反映する。

mod cross_shareholdings;
mod large_volume_shareholdings;
mod major_shareholders;

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, NaiveDate};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::data_provider::jquants::JQuantsClient;
use crate::data_provider::{DataProvider, DataProviderKind};

/// ポーリング実行間隔。リアルタイム性を求めない pull 型運用のプロダクト方針に基づき 1 日間隔とする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 既存データの最新 sub_date からこの日数分遡って再取得する幅。EDINET の訂正報告書は
/// 差分取得 (cursor) の仕組みが無く、同じ DocId への上書きとしてのみ反映されるため、
/// 前回サイクル以降に生じた訂正を拾うために一定期間を再取得する。
const REFETCH_WINDOW_DAYS: i64 = 30;

/// 1 サイクルの取り込み結果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestStats {
    pub days_processed: usize,
    pub documents_saved: usize,
    /// 同じ cycle 内の再試行 (2 回目のパス) でも取得に失敗した日数。この日付は
    /// `latest_sub_date` 基準の再取得対象からも外れるため、書類が永久に欠落する。
    pub failed_dates: usize,
}

/// EDINET 取り込み対象の 1 エンドポイント (テーブル) が実装する設定。
trait EdinetEndpoint {
    /// ログ用の名前
    const NAME: &'static str;
    /// J-Quants API のパス (例: "/edinet/large-volume-shareholders")
    const PATH: &'static str;
    /// このエンドポイントのデータ提供開始日
    fn available_from() -> NaiveDate;
    /// 既存データの最新 sub_date (テーブルが空なら None)
    async fn latest_sub_date(db: &DatabaseConnection) -> Result<Option<NaiveDate>, sea_orm::DbErr>;
    /// 1 日分のドキュメント配列を doc_id で upsert し、保存件数を返す
    async fn upsert(db: &DatabaseConnection, docs: Vec<Value>) -> Result<usize, sea_orm::DbErr>;
}

/// 書類オブジェクトから DB カラムに必要な最小限のフィールドを取り出す。
pub(crate) struct DocumentMeta {
    doc_id: String,
    code: Option<String>,
    edinet_code: String,
    sub_date: NaiveDate,
}

/// `doc` から `DocumentMeta` を取り出す。`DocId` / `EdinetCode` / `SubDate` のいずれかが
/// 欠落・不正な形式の場合は警告ログを出して `None` を返す (その 1 件だけスキップし、
/// 同じ日の他の書類の取り込みは継続する)。
fn extract_meta(doc: &Value) -> Option<DocumentMeta> {
    let doc_id = doc.get("DocId").and_then(Value::as_str);
    let edinet_code = doc.get("EdinetCode").and_then(Value::as_str);
    let sub_date_str = doc.get("SubDate").and_then(Value::as_str);

    let (Some(doc_id), Some(edinet_code), Some(sub_date_str)) = (doc_id, edinet_code, sub_date_str)
    else {
        tracing::warn!(
            ?doc,
            "DocId/EdinetCode/SubDate のいずれかが欠落、この書類をスキップします"
        );
        return None;
    };

    let sub_date = match NaiveDate::parse_from_str(sub_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(doc_id, sub_date_str, error = %e, "SubDate のパースに失敗、この書類をスキップします");
            return None;
        }
    };

    // J-Quants API が返す Code は 5 桁のまま保存する (stock/instruments 等の既存 4 桁との
    // 突き合わせは、この列を読み出す側の責務のため、ここでは行わない)
    let code = doc.get("Code").and_then(Value::as_str).map(str::to_string);

    Some(DocumentMeta {
        doc_id: doc_id.to_string(),
        code,
        edinet_code: edinet_code.to_string(),
        sub_date,
    })
}

/// `docs` から `DocumentMeta` を取り出せた書類だけを `build` で `E::ActiveModel` に変換し、
/// `doc_id` の重複を `conflict` の指定で upsert する。3 エンドポイントの `upsert` 実装が
/// entity の型名以外まったく同一なため、ここに共通化する。
pub(crate) async fn upsert_documents<E>(
    db: &DatabaseConnection,
    docs: Vec<Value>,
    build: impl Fn(DocumentMeta, Value) -> E::ActiveModel,
    conflict: sea_orm::sea_query::OnConflict,
) -> Result<usize, sea_orm::DbErr>
where
    E: sea_orm::EntityTrait,
    E::ActiveModel: Send,
{
    let models: Vec<E::ActiveModel> = docs
        .iter()
        .filter_map(|doc| extract_meta(doc).map(|meta| build(meta, doc.clone())))
        .collect();

    if models.is_empty() {
        return Ok(0);
    }
    let count = models.len();

    E::insert_many(models)
        .on_conflict(conflict)
        .exec_without_returning(db)
        .await?;

    Ok(count)
}

/// `sub_date_column` で降順ソートした最新 1 行から `sub_date` を取り出す。3 エンドポイントの
/// `latest_sub_date` 実装が entity の型名以外まったく同一なため、ここに共通化する。
pub(crate) async fn latest_sub_date_of<E, C>(
    db: &DatabaseConnection,
    sub_date_column: C,
    sub_date: impl Fn(&E::Model) -> NaiveDate,
) -> Result<Option<NaiveDate>, sea_orm::DbErr>
where
    E: sea_orm::EntityTrait,
    C: sea_orm::ColumnTrait,
{
    use sea_orm::QueryOrder;

    let latest = E::find().order_by_desc(sub_date_column).one(db).await?;
    Ok(latest.map(|m| sub_date(&m)))
}

/// 取得可能範囲・既存データの最新 sub_date から、この cycle で取得する日付範囲を決め、
/// 1 日ずつ書類を取得して DB に保存する。契約プランが未検出 (`known_fetchable_range` が
/// `None`) ならスキップする。取得に失敗した日付はメインループでは記録するだけにして
/// ループを最後まで走らせ、完了後に同じ cycle 内でその日付だけ再取得する (`start` は
/// 次 cycle 時点の `latest_sub_date` から決まるため、次 cycle を待つと `REFETCH_WINDOW_DAYS`
/// を過ぎた時点で再取得対象から外れてしまう)。
async fn run_ingest_cycle<T: EdinetEndpoint>(
    db: &DatabaseConnection,
    client: &JQuantsClient,
) -> Result<IngestStats, sea_orm::DbErr> {
    let Some((plan_from, plan_to)) = client.known_fetchable_range() else {
        tracing::info!(
            endpoint = T::NAME,
            "契約プランが未検出のため EDINET 取り込みをスキップします"
        );
        return Ok(IngestStats::default());
    };

    let earliest = plan_from.max(T::available_from());
    let start = match T::latest_sub_date(db).await? {
        Some(latest) => (latest - ChronoDuration::days(REFETCH_WINDOW_DAYS)).max(earliest),
        None => earliest,
    };

    let mut stats = IngestStats::default();
    let mut retry_dates = Vec::new();
    let mut date = start;
    while date <= plan_to {
        match client.fetch_edinet_documents(T::PATH, date).await {
            Ok(docs) if !docs.is_empty() => {
                stats.documents_saved += T::upsert(db, docs).await?;
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(endpoint = T::NAME, %date, error = %e, "EDINET 書類の取得に失敗、この cycle 終了後に再試行します");
                retry_dates.push(date);
            }
        }
        stats.days_processed += 1;
        date += ChronoDuration::days(1);
    }

    for date in retry_dates {
        match client.fetch_edinet_documents(T::PATH, date).await {
            Ok(docs) if !docs.is_empty() => {
                stats.documents_saved += T::upsert(db, docs).await?;
            }
            Ok(_) => {}
            Err(e) => {
                stats.failed_dates += 1;
                tracing::error!(
                    endpoint = T::NAME,
                    %date,
                    error = %e,
                    "EDINET 書類の再取得にも失敗しました。latest_sub_date 基準の再取得対象からも外れるため、この日付の書類は取り込めていません"
                );
            }
        }
    }

    Ok(stats)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す。
/// IBKR には EDINET 相当のデータが無いため、JQuants 以外の provider では何もしない。
pub fn spawn_poll(
    db: DatabaseConnection,
    provider: Arc<DataProviderKind>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let DataProviderKind::JQuants(client) = provider.as_ref() else {
                continue;
            };
            run_all(&db, client).await;
        }
    })
}

async fn run_all(db: &DatabaseConnection, client: &JQuantsClient) {
    let results = [
        (
            large_volume_shareholdings::Endpoint::NAME,
            run_ingest_cycle::<large_volume_shareholdings::Endpoint>(db, client).await,
        ),
        (
            cross_shareholdings::Endpoint::NAME,
            run_ingest_cycle::<cross_shareholdings::Endpoint>(db, client).await,
        ),
        (
            major_shareholders::Endpoint::NAME,
            run_ingest_cycle::<major_shareholders::Endpoint>(db, client).await,
        ),
    ];
    for (name, result) in results {
        match result {
            Ok(stats) => {
                tracing::debug!(endpoint = name, ?stats, "EDINET ingest cycle completed")
            }
            Err(err) => tracing::warn!(endpoint = name, %err, "EDINET ingest cycle failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;
    use sqlx::PgPool;

    use super::*;
    use crate::data_provider::jquants::mock::JQuantsMockServer;
    use crate::entities::edinet_large_volume_shareholdings;
    use crate::testing::create_test_db;

    // --- extract_meta ---

    #[rstest]
    #[case::ok(
        json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        }),
        Some(("S100ABCD", Some("72030"), "E00001", "2025-01-06"))
    )]
    #[case::missing_doc_id(
        json!({
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        }),
        None
    )]
    #[case::missing_edinet_code(
        json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "SubDate": "2025-01-06",
        }),
        None
    )]
    #[case::missing_sub_date(
        json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
        }),
        None
    )]
    #[case::invalid_sub_date_format(
        json!({
            "DocId": "S100ABCD",
            "Code": "72030",
            "EdinetCode": "E00001",
            "SubDate": "20250106",
        }),
        None
    )]
    #[case::missing_code_becomes_none(
        json!({
            "DocId": "S100ABCD",
            "EdinetCode": "E00001",
            "SubDate": "2025-01-06",
        }),
        Some(("S100ABCD", None, "E00001", "2025-01-06"))
    )]
    fn extract_meta_cases(
        #[case] doc: Value,
        #[case] expected: Option<(&str, Option<&str>, &str, &str)>,
    ) {
        let actual = extract_meta(&doc).map(|meta| {
            (
                meta.doc_id,
                meta.code,
                meta.edinet_code,
                meta.sub_date.format("%Y-%m-%d").to_string(),
            )
        });
        let expected = expected.map(|(doc_id, code, edinet_code, sub_date)| {
            (
                doc_id.to_string(),
                code.map(str::to_string),
                edinet_code.to_string(),
                sub_date.to_string(),
            )
        });
        assert_eq!(actual, expected);
    }

    // --- run_ingest_cycle (large_volume_shareholdings で代表して検証) ---

    fn doc(doc_id: &str, code: &str, edinet_code: &str, sub_date: &str) -> Value {
        json!({
            "DocId": doc_id,
            "Code": code,
            "EdinetCode": edinet_code,
            "SubDate": sub_date,
        })
    }

    async fn find_all(db: &DatabaseConnection) -> Vec<edinet_large_volume_shareholdings::Model> {
        use sea_orm::EntityTrait;
        edinet_large_volume_shareholdings::Entity::find()
            .all(db)
            .await
            .expect("find all")
    }

    #[sqlx::test(migrations = false)]
    async fn fetches_from_available_from_when_table_is_empty(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;

        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(2);

        // 3 日分すべてに書類を用意する (available_from, +1, +2)
        for offset in 0..=2 {
            let date = from + ChronoDuration::days(offset);
            mock.edinet_documents("/edinet/large-volume-shareholders")
                .date(&date.format("%Y%m%d").to_string())
                .docs(vec![doc(
                    &format!("S{offset}"),
                    "72030",
                    "E00001",
                    &date.format("%Y-%m-%d").to_string(),
                )])
                .ok()
                .await;
        }

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));

        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle ok");

        assert_eq!(
            stats,
            IngestStats {
                days_processed: 3,
                documents_saved: 3,
                failed_dates: 0,
            }
        );

        let rows = find_all(&db).await;
        assert_eq!(rows.len(), 3);
    }

    #[sqlx::test(migrations = false)]
    async fn refetches_from_latest_sub_date_minus_window_on_second_cycle(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;

        // latest を available_from から離しておき、素朴に available_from から
        // 取得し直すロジックとは区別できるようにする。
        let from = large_volume_shareholdings::Endpoint::available_from();
        let latest = from + ChronoDuration::days(60);
        let expected_start = latest - ChronoDuration::days(REFETCH_WINDOW_DAYS);
        let to = expected_start + ChronoDuration::days(1);

        // 既存データの最新 sub_date を作る
        large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![doc(
                "S-EXISTING",
                "72030",
                "E00001",
                &latest.format("%Y-%m-%d").to_string(),
            )],
        )
        .await
        .expect("seed existing row");

        // 再取得範囲 (expected_start ..= to) の全日に空レスポンスを用意する
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
            .expect("cycle ok");

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
    async fn skips_when_plan_is_undetected(pool: PgPool) {
        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;
        let client = mock.client().expect("client");

        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle ok");

        assert_eq!(stats, IngestStats::default());
        assert_eq!(find_all(&db).await.len(), 0);
    }

    #[sqlx::test(migrations = false)]
    async fn upserting_same_doc_id_twice_keeps_a_single_updated_row(pool: PgPool) {
        let db = create_test_db(pool).await;

        let saved = large_volume_shareholdings::Endpoint::upsert(
            &db,
            vec![doc("S100ABCD", "72030", "E00001", "2025-01-06")],
        )
        .await
        .expect("first upsert");
        assert_eq!(saved, 1);

        let second_doc = doc("S100ABCD", "67580", "E00001", "2025-01-06");
        let saved = large_volume_shareholdings::Endpoint::upsert(&db, vec![second_doc.clone()])
            .await
            .expect("second upsert");
        assert_eq!(saved, 1);

        let fixed: chrono::DateTime<chrono::FixedOffset> =
            chrono::DateTime::UNIX_EPOCH.fixed_offset();
        let rows: Vec<edinet_large_volume_shareholdings::Model> = find_all(&db)
            .await
            .into_iter()
            .map(|mut m| {
                m.created_at = fixed;
                m.updated_at = fixed;
                m
            })
            .collect();

        assert_eq!(
            rows,
            vec![edinet_large_volume_shareholdings::Model {
                doc_id: "S100ABCD".to_string(),
                code: Some("67580".to_string()),
                edinet_code: "E00001".to_string(),
                sub_date: NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                document: second_doc,
                created_at: fixed,
                updated_at: fixed,
            }]
        );
    }

    // --- run_ingest_cycle の同一 cycle 内リトライ ---

    #[sqlx::test(migrations = false)]
    async fn retries_a_failed_date_within_the_same_cycle_and_recovers(pool: PgPool) {
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, ResponseTemplate};

        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;

        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(1);
        let recovered_doc = doc(
            "S-RECOVERED",
            "72030",
            "E00001",
            &from.format("%Y-%m-%d").to_string(),
        );

        // from 日: 1 回目 (メインループ) は 403 (リトライ非対象のエラー) で失敗し、
        // 2 回目 (cycle 内の再試行) で成功する
        Mock::given(method("GET"))
            .and(path(large_volume_shareholdings::Endpoint::PATH))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .up_to_n_times(1)
            .mount(mock.server_ref())
            .await;
        mock.edinet_documents(large_volume_shareholdings::Endpoint::PATH)
            .date(&from.format("%Y%m%d").to_string())
            .docs(vec![recovered_doc])
            .ok()
            .await;

        // to 日: 通常成功 (空)
        mock.edinet_documents(large_volume_shareholdings::Endpoint::PATH)
            .date(&to.format("%Y%m%d").to_string())
            .docs(vec![])
            .ok()
            .await;

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));

        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle ok");

        assert_eq!(
            stats,
            IngestStats {
                days_processed: 2,
                documents_saved: 1,
                failed_dates: 0,
            }
        );

        let rows = find_all(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].doc_id, "S-RECOVERED");
    }

    #[sqlx::test(migrations = false)]
    async fn counts_failed_dates_when_the_retry_also_fails(pool: PgPool) {
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, ResponseTemplate};

        let db = create_test_db(pool).await;
        let mock = JQuantsMockServer::start().await;

        let from = large_volume_shareholdings::Endpoint::available_from();
        let to = from + ChronoDuration::days(1);

        // from 日: 何度リクエストしても失敗する
        Mock::given(method("GET"))
            .and(path(large_volume_shareholdings::Endpoint::PATH))
            .and(query_param("date", from.format("%Y%m%d").to_string()))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(json!({ "message": "Forbidden" })),
            )
            .mount(mock.server_ref())
            .await;

        // to 日: 通常成功 (空)
        mock.edinet_documents(large_volume_shareholdings::Endpoint::PATH)
            .date(&to.format("%Y%m%d").to_string())
            .docs(vec![])
            .ok()
            .await;

        let client = mock.client().expect("client");
        client.set_detected_range((from, to));

        let stats = run_ingest_cycle::<large_volume_shareholdings::Endpoint>(&db, &client)
            .await
            .expect("cycle ok");

        assert_eq!(
            stats,
            IngestStats {
                days_processed: 2,
                documents_saved: 0,
                failed_dates: 1,
            }
        );

        assert_eq!(find_all(&db).await.len(), 0);
    }
}
