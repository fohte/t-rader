//! `stock.sector_id` または `stock.product_category` が未設定の銘柄を対象に
//! `DataProvider` から業種・商品区分を取得して補完する定期タスク。
//!
//! 取得できなかったフィールドは NULL のまま残り、次サイクルで再試行される。

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::ActiveValue::{NotSet, Set, Unchanged};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tokio::task::JoinHandle;

use crate::data_provider::{DataProvider, DataProviderKind};
use crate::entities::{sector, stock};

/// poll task のデフォルト実行間隔。
/// sector_id や product_category が NULL になるのは新規銘柄追加時のみで定常状態では稀なため、news
/// (1 時間) ほどの高頻度は不要。リアルタイム性を重視せず pull 型で「開いて読む」プロダクト方針も
/// 踏まえ、1 日間隔にする。
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// 1 サイクルで処理する対象銘柄数の上限。
/// JQuantsClient の RateLimiter (5 req/60s) はウォッチリスト追加時の日足取得等と共有のため、
/// 1 サイクルで専有しすぎないよう小さい値に抑える。未処理分は次サイクルに繰り越される。
const BATCH_LIMIT: u64 = 5;

/// poll サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackfillStats {
    pub attempted: usize,
    pub filled: usize,
}

/// `sector_id` または `product_category` が NULL の stock を対象に業種・商品区分を取得し、
/// 埋められた分だけ反映する。
pub async fn run_backfill_cycle<P: DataProvider>(
    db: &DatabaseConnection,
    provider: &P,
) -> Result<BackfillStats, sea_orm::DbErr> {
    let targets = stock::Entity::find()
        .filter(
            Condition::any()
                .add(stock::Column::SectorId.is_null())
                .add(stock::Column::ProductCategory.is_null()),
        )
        .order_by_asc(stock::Column::UpdatedAt)
        .limit(BATCH_LIMIT)
        .all(db)
        .await?;

    if targets.len() as u64 == BATCH_LIMIT {
        tracing::debug!(
            batch_limit = BATCH_LIMIT,
            "instrument backfill 対象がバッチ上限に到達、次サイクルに繰り越し"
        );
    }

    let mut stats = BackfillStats::default();
    for target in targets {
        stats.attempted += 1;

        let instrument = match provider.fetch_instrument(&target.id).await {
            Ok(i) => i,
            Err(err) => {
                tracing::warn!(stock_id = %target.id, %err, "instrument 情報取得に失敗、次サイクルで再試行");
                let touch = stock::ActiveModel {
                    id: Unchanged(target.id),
                    sector_id: NotSet,
                    product_category: NotSet,
                    updated_at: Set(Utc::now().fixed_offset()),
                    name: NotSet,
                    market: NotSet,
                    created_at: NotSet,
                };
                stock::Entity::update(touch).exec(db).await?;
                continue;
            }
        };

        let sector_name = if target.sector_id.is_none() {
            instrument.sector.filter(|s| !s.trim().is_empty())
        } else {
            None
        };
        if let Some(name) = &sector_name {
            upsert_sector(db, name).await?;
        }

        let product_category = if target.product_category.is_none() {
            instrument.product_category.filter(|s| !s.trim().is_empty())
        } else {
            None
        };

        if sector_name.is_none() && product_category.is_none() {
            tracing::debug!(stock_id = %target.id, "業種・商品区分とも取得できず、NULL のまま");
            continue;
        }

        let active = stock::ActiveModel {
            id: Unchanged(target.id),
            sector_id: sector_name.map_or(NotSet, |s| Set(Some(s))),
            product_category: product_category.map_or(NotSet, |p| Set(Some(p))),
            updated_at: Set(Utc::now().fixed_offset()),
            name: NotSet,
            market: NotSet,
            created_at: NotSet,
        };
        stock::Entity::update(active).exec(db).await?;
        stats.filled += 1;
    }

    Ok(stats)
}

/// 業種名を id および name として sector を upsert する
async fn upsert_sector(db: &DatabaseConnection, name: &str) -> Result<(), sea_orm::DbErr> {
    let model = sector::ActiveModel {
        id: Set(name.to_string()),
        name: Set(name.to_string()),
    };
    // support_returning() が true (Postgres) だと通常の exec() は RETURNING 行を
    // 前提にしており、ON CONFLICT DO NOTHING で行が返らないと DbErr::RecordNotInserted
    // になってしまう。conflict を正常系として扱うため returning 不要な exec を使う
    sector::Entity::insert(model)
        .on_conflict(
            OnConflict::column(sector::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
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
            match run_backfill_cycle(&db, provider.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        attempted = stats.attempted,
                        filled = stats.filled,
                        "instrument backfill cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "instrument backfill cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_provider::DataProviderError;
    use crate::models::instrument::{Instrument, Market};
    use crate::testing::create_test_db;
    use sea_orm::ActiveModelTrait;
    use sqlx::PgPool;

    /// テスト用のモックデータプロバイダー。登録済みの id のみ成功を返し、
    /// 未登録の id は NotFound エラーとして扱う (取得失敗ケースを表現する)。
    struct MockProvider {
        instruments: Vec<Instrument>,
    }

    impl DataProvider for MockProvider {
        async fn fetch_daily_bars(
            &self,
            _instrument_id: &str,
            _range: &crate::data_provider::DateRange,
        ) -> Result<Vec<crate::models::bar::Bar>, DataProviderError> {
            unimplemented!("instrument backfill テストでは使わない")
        }

        async fn fetch_instrument(
            &self,
            instrument_id: &str,
        ) -> Result<Instrument, DataProviderError> {
            self.instruments
                .iter()
                .find(|i| i.id == instrument_id)
                .cloned()
                .ok_or_else(|| DataProviderError::NotFound(instrument_id.to_string()))
        }
    }

    /// テスト用の stock 行を作成する。`sector_id` を指定する場合、FK 制約を満たすため
    /// 対応する sector 行を事前に upsert する。
    async fn insert_stock(
        db: &DatabaseConnection,
        id: &str,
        name: &str,
        sector_id: Option<&str>,
        product_category: Option<&str>,
    ) {
        if let Some(name) = sector_id {
            upsert_sector(db, name).await.expect("seed sector");
        }
        stock::ActiveModel {
            id: Set(id.to_string()),
            name: Set(name.to_string()),
            market: Set(None),
            sector_id: Set(sector_id.map(|s| s.to_string())),
            product_category: Set(product_category.map(|s| s.to_string())),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test stock");
    }

    fn instrument(id: &str, sector: Option<&str>, product_category: Option<&str>) -> Instrument {
        Instrument {
            id: id.to_string(),
            name: format!("Test {id}"),
            market: Market::Tse,
            sector: sector.map(|s| s.to_string()),
            product_category: product_category.map(|s| s.to_string()),
        }
    }

    async fn fetch_stock(db: &DatabaseConnection, stock_id: &str) -> stock::Model {
        stock::Entity::find_by_id(stock_id.to_string())
            .one(db)
            .await
            .expect("query ok")
            .expect("stock exists")
    }

    #[sqlx::test(migrations = false)]
    async fn fills_sector_and_product_category_when_both_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "7203", "トヨタ自動車", None, None).await;

        let provider = MockProvider {
            instruments: vec![instrument("7203", Some("輸送用機器"), Some("111"))],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let updated = fetch_stock(&db, "7203").await;
        let sector_row = sector::Entity::find_by_id("輸送用機器".to_string())
            .one(&db)
            .await
            .expect("query ok");

        assert_eq!(
            (
                stats,
                updated.sector_id,
                updated.product_category,
                sector_row.map(|s| s.name),
            ),
            (
                BackfillStats {
                    attempted: 1,
                    filled: 1,
                },
                Some("輸送用機器".to_string()),
                Some("111".to_string()),
                Some("輸送用機器".to_string()),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn leaves_both_null_when_provider_has_neither(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "9999", "無業種銘柄", None, None).await;

        let provider = MockProvider {
            instruments: vec![instrument("9999", None, None)],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let updated = fetch_stock(&db, "9999").await;

        assert_eq!(
            (stats, updated.sector_id, updated.product_category),
            (
                BackfillStats {
                    attempted: 1,
                    filled: 0,
                },
                None,
                None,
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn fills_only_product_category_when_sector_already_set(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "8888", "既に業種あり", Some("情報・通信業"), None).await;

        let provider = MockProvider {
            instruments: vec![instrument("8888", Some("その他"), Some("111"))],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let updated = fetch_stock(&db, "8888").await;

        assert_eq!(
            (stats, updated.sector_id, updated.product_category),
            (
                BackfillStats {
                    attempted: 1,
                    filled: 1,
                },
                Some("情報・通信業".to_string()),
                Some("111".to_string()),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn fills_only_sector_when_product_category_already_set(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "7203", "トヨタ自動車", None, Some("111")).await;

        let provider = MockProvider {
            instruments: vec![instrument("7203", Some("輸送用機器"), Some("999"))],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let updated = fetch_stock(&db, "7203").await;

        assert_eq!(
            (stats, updated.sector_id, updated.product_category),
            (
                BackfillStats {
                    attempted: 1,
                    filled: 1,
                },
                Some("輸送用機器".to_string()),
                Some("111".to_string()),
            )
        );
    }

    #[sqlx::test(migrations = false)]
    async fn ignores_stock_that_already_has_both_fields(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "8888", "両方あり", Some("情報・通信業"), Some("111")).await;

        let provider = MockProvider {
            instruments: vec![instrument("8888", Some("その他"), Some("999"))],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");

        assert_eq!(
            stats,
            BackfillStats {
                attempted: 0,
                filled: 0,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn skips_stock_when_provider_errors(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "1111", "取得失敗銘柄", None, None).await;

        let provider = MockProvider {
            instruments: vec![],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let updated = fetch_stock(&db, "1111").await;

        assert_eq!(
            (stats, updated.sector_id, updated.product_category),
            (
                BackfillStats {
                    attempted: 1,
                    filled: 0,
                },
                None,
                None,
            )
        );
    }

    /// 取得失敗時も updated_at を更新しないと、対象クエリが ORDER BY updated_at ASC で
    /// 並ぶ際に恒久的に失敗する銘柄が先頭を占有し続け、他の対象が処理されなくなる
    #[sqlx::test(migrations = false)]
    async fn touches_updated_at_when_provider_fetch_fails(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "1111", "取得失敗銘柄", None, None).await;
        let before = fetch_stock(&db, "1111").await;

        let provider = MockProvider {
            instruments: vec![],
        };

        run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let after = fetch_stock(&db, "1111").await;

        assert!(after.updated_at > before.updated_at);
    }

    /// 2 件目の upsert_sector は同じ id への ON CONFLICT DO NOTHING を踏む。conflict を
    /// エラー扱いしてしまうと、後続銘柄がまとめて未処理になる (upsert_sector の doc 参照)
    #[sqlx::test(migrations = false)]
    async fn fills_sector_id_for_multiple_stocks_sharing_the_same_sector(pool: PgPool) {
        let db = create_test_db(pool).await;
        insert_stock(&db, "7203", "トヨタ自動車", None, None).await;
        insert_stock(&db, "7267", "ホンダ", None, None).await;

        let provider = MockProvider {
            instruments: vec![
                instrument("7203", Some("輸送用機器"), None),
                instrument("7267", Some("輸送用機器"), None),
            ],
        };

        let stats = run_backfill_cycle(&db, &provider).await.expect("cycle ok");
        let stock_7203 = fetch_stock(&db, "7203").await;
        let stock_7267 = fetch_stock(&db, "7267").await;

        assert_eq!(
            (stats, stock_7203.sector_id, stock_7267.sector_id),
            (
                BackfillStats {
                    attempted: 2,
                    filled: 2,
                },
                Some("輸送用機器".to_string()),
                Some("輸送用機器".to_string()),
            )
        );
    }
}
