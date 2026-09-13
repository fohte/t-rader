//! 戦略実行 MCP の `read_sector_short_ratio` tool。`short_ratio` (J-Quants
//! `/markets/short-ratio` の業種別空売り比率) を業種名・期間で読む。
//!
//! `short_ratio.sector33_code` は 33 業種コードのまま保持されているが、tool の入力は
//! `sector` テーブル / `search_refs` / `check_buyable_qty` と同じ業種名で受け取る
//! (プロジェクト方針)。33 業種は東証の固定分類のため、対応表はここに定数で持つ。
//! 空売り比率の定義 (空売り (価格規制あり+なし) の売買代金 / 実注文と空売りを合わせた
//! 売買代金) は JPX の空売り集計公表ページに基づく。戦略に属さない市場データのため
//! `search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には使わない。

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::short_ratio;

use super::dto::{ReadSectorShortRatioParams, ReadSectorShortRatioResult, SectorShortRatioDto};
use super::{StrategyServer, clamp_limit, db_error, decimal_to_f64, invalid_params};

/// 33 業種名 -> 33 業種コード。
/// https://jpx-jquants.com/ja/spec/eq-master/sector33code
const SECTOR33_CODES: &[(&str, &str)] = &[
    ("水産・農林業", "0050"),
    ("鉱業", "1050"),
    ("建設業", "2050"),
    ("食料品", "3050"),
    ("繊維製品", "3100"),
    ("パルプ・紙", "3150"),
    ("化学", "3200"),
    ("医薬品", "3250"),
    ("石油･石炭製品", "3300"),
    ("ゴム製品", "3350"),
    ("ガラス･土石製品", "3400"),
    ("鉄鋼", "3450"),
    ("非鉄金属", "3500"),
    ("金属製品", "3550"),
    ("機械", "3600"),
    ("電気機器", "3650"),
    ("輸送用機器", "3700"),
    ("精密機器", "3750"),
    ("その他製品", "3800"),
    ("電気･ガス業", "4050"),
    ("陸運業", "5050"),
    ("海運業", "5100"),
    ("空運業", "5150"),
    ("倉庫･運輸関連業", "5200"),
    ("情報･通信業", "5250"),
    ("卸売業", "6050"),
    ("小売業", "6100"),
    ("銀行業", "7050"),
    ("証券･商品先物取引業", "7100"),
    ("保険業", "7150"),
    ("その他金融業", "7200"),
    ("不動産業", "8050"),
    ("サービス業", "9050"),
    ("その他", "9999"),
];

fn sector33_code_for_name(sector: &str) -> Option<&'static str> {
    SECTOR33_CODES
        .iter()
        .find(|(name, _)| *name == sector)
        .map(|(_, code)| *code)
}

/// 空売り比率 = 空売り (価格規制あり+なし) の売買代金 / (実注文+空売り) の売買代金合計。
/// いずれかが null (売買が無い日) なら null。合計が 0 のときも 0 除算を避けて null にする。
fn compute_short_ratio(
    sell_excluding_short: Option<Decimal>,
    with_restriction: Option<Decimal>,
    without_restriction: Option<Decimal>,
) -> Option<f64> {
    let sell_excluding_short = sell_excluding_short?;
    let with_restriction = with_restriction?;
    let without_restriction = without_restriction?;
    let short = with_restriction + without_restriction;
    let total = sell_excluding_short + short;
    if total.is_zero() {
        return None;
    }
    Some(decimal_to_f64(short / total))
}

fn sector_short_ratio_dto(row: short_ratio::Model) -> SectorShortRatioDto {
    let short_ratio = compute_short_ratio(
        row.sell_excluding_short_value,
        row.short_with_restriction_value,
        row.short_without_restriction_value,
    );
    SectorShortRatioDto {
        date: row.date,
        sell_excluding_short_value: row.sell_excluding_short_value.map(decimal_to_f64),
        short_with_restriction_value: row.short_with_restriction_value.map(decimal_to_f64),
        short_without_restriction_value: row.short_without_restriction_value.map(decimal_to_f64),
        short_ratio,
    }
}

impl StrategyServer {
    pub(crate) async fn read_sector_short_ratio_inner(
        &self,
        _session_strategy_id: Uuid,
        params: ReadSectorShortRatioParams,
    ) -> Result<ReadSectorShortRatioResult, McpError> {
        let sector33_code = sector33_code_for_name(&params.sector)
            .ok_or_else(|| invalid_params(format!("unknown sector name: {:?}", params.sector)))?;
        let limit = clamp_limit(params.limit);

        let mut query =
            short_ratio::Entity::find().filter(short_ratio::Column::Sector33Code.eq(sector33_code));
        if let Some(from) = params.from {
            query = query.filter(short_ratio::Column::Date.gte(from));
        }
        if let Some(to) = params.to {
            query = query.filter(short_ratio::Column::Date.lte(to));
        }

        let rows = query
            .order_by_desc(short_ratio::Column::Date)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        Ok(ReadSectorShortRatioResult {
            sector: params.sector,
            items: rows.into_iter().map(sector_short_ratio_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sea_orm::DatabaseConnection;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::short_ratio;
    use crate::testing::create_test_db;

    use super::super::dto::{
        ReadSectorShortRatioParams, ReadSectorShortRatioResult, SectorShortRatioDto,
    };
    use super::super::tests_common::build_server;
    use super::{compute_short_ratio, sector33_code_for_name};

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn dec(s: &str) -> Decimal {
        s.parse().expect("valid decimal")
    }

    #[rstest]
    #[case::plain_name("輸送用機器", Some("3700"))]
    #[case::halfwidth_dot_name("石油･石炭製品", Some("3300"))]
    #[case::catch_all("その他", Some("9999"))]
    #[case::unknown("半導体", None)]
    fn sector33_code_for_name_cases(#[case] sector: &str, #[case] expected: Option<&str>) {
        assert_eq!(sector33_code_for_name(sector), expected);
    }

    #[rstest]
    #[case::no_trading(None, None, None, None)]
    #[case::computes_ratio(Some(dec("700")), Some(dec("200")), Some(dec("100")), Some(0.3))]
    #[case::zero_total_is_none(Some(dec("0")), Some(dec("0")), Some(dec("0")), None)]
    fn compute_short_ratio_cases(
        #[case] sell_excluding_short: Option<Decimal>,
        #[case] with_restriction: Option<Decimal>,
        #[case] without_restriction: Option<Decimal>,
        #[case] expected: Option<f64>,
    ) {
        assert_eq!(
            compute_short_ratio(sell_excluding_short, with_restriction, without_restriction),
            expected
        );
    }

    async fn seed(
        db: &DatabaseConnection,
        sector33_code: &str,
        date: NaiveDate,
        values: Option<(&str, &str, &str)>,
    ) {
        let (sell, with_r, without_r) = match values {
            Some((sell, with_r, without_r)) => {
                (Some(dec(sell)), Some(dec(with_r)), Some(dec(without_r)))
            }
            None => (None, None, None),
        };
        short_ratio::ActiveModel {
            date: Set(date),
            sector33_code: Set(sector33_code.to_string()),
            sell_excluding_short_value: Set(sell),
            short_with_restriction_value: Set(with_r),
            short_without_restriction_value: Set(without_r),
        }
        .insert(db)
        .await
        .expect("seed short ratio");
    }

    #[sqlx::test(migrations = false)]
    async fn rejects_unknown_sector_name(pool: PgPool) {
        let db = create_test_db(pool).await;

        let err = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "半導体".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect_err("unknown sector name should be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn returns_matching_sector_newest_first_with_computed_ratio(pool: PgPool) {
        let db = create_test_db(pool).await;
        seed(&db, "3700", ymd(2026, 1, 5), Some(("700", "200", "100"))).await;
        seed(&db, "3700", ymd(2026, 1, 6), None).await;
        seed(&db, "3650", ymd(2026, 1, 6), Some(("100", "50", "50"))).await;

        let result = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "輸送用機器".to_string(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect("read_sector_short_ratio");

        assert_eq!(
            result,
            ReadSectorShortRatioResult {
                sector: "輸送用機器".to_string(),
                items: vec![
                    SectorShortRatioDto {
                        date: ymd(2026, 1, 6),
                        sell_excluding_short_value: None,
                        short_with_restriction_value: None,
                        short_without_restriction_value: None,
                        short_ratio: None,
                    },
                    SectorShortRatioDto {
                        date: ymd(2026, 1, 5),
                        sell_excluding_short_value: Some(700.0),
                        short_with_restriction_value: Some(200.0),
                        short_without_restriction_value: Some(100.0),
                        short_ratio: Some(0.3),
                    },
                ],
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn filters_by_date_range_and_respects_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        for day in [1u32, 2, 3, 4] {
            seed(&db, "3700", ymd(2026, 1, day), Some(("100", "10", "10"))).await;
        }

        let result = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "輸送用機器".to_string(),
                    from: Some(ymd(2026, 1, 2)),
                    to: Some(ymd(2026, 1, 3)),
                    limit: Some(1),
                },
            )
            .await
            .expect("read_sector_short_ratio");

        assert_eq!(
            result.items.iter().map(|i| i.date).collect::<Vec<_>>(),
            vec![ymd(2026, 1, 3)],
        );
    }
}
