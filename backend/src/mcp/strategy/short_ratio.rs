//! 戦略実行 MCP の `read_sector_short_ratio` tool。`short_ratio` (J-Quants
//! `/markets/short-ratio` の業種別空売り比率) をグループ key・期間で読む。
//!
//! tool の入力は `sync_source = 'jquants'` の分類軸にあるグループ key で受け取る。
//! `short_ratio.sector33_code` は市場データ側のコードのため、対応する同期元コードを
//! stock_group から取得する。
//! 空売り比率の定義 (空売り (価格規制あり+なし) の売買代金 / 実注文と空売りを合わせた
//! 売買代金) は JPX の空売り集計公表ページに基づく。戦略に属さない市場データのため
//! `search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には使わない。

use core_application::short_ratio::ShortRatioQuery;
use core_application::stock_group::StockGroupSyncSourceCodeLookup;
use core_application::strategy_scope::StrategyScope;
use core_domain::short_ratio::ShortRatio;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;

use super::dto::{ReadSectorShortRatioParams, ReadSectorShortRatioResult, SectorShortRatioDto};
use super::{StrategyServer, clamp_limit, decimal_to_f64, internal_error, invalid_params};

const JQUANTS_SYNC_SOURCE: &str = "jquants";

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

fn sector_short_ratio_dto(row: ShortRatio) -> SectorShortRatioDto {
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
        scope: impl Into<StrategyScope>,
        params: ReadSectorShortRatioParams,
    ) -> Result<ReadSectorShortRatioResult, McpError> {
        let scope = scope.into();
        let sector33_code = self
            .dependencies
            .stock_groups
            .find_sync_source_code(JQUANTS_SYNC_SOURCE, &params.sector)
            .await
            .map_err(|error| {
                tracing::error!(%error, "strategy mcp stock group lookup failed");
                internal_error(format!("database error: {error}"))
            })?;
        let sector33_code = match sector33_code {
            StockGroupSyncSourceCodeLookup::NotFound => {
                return Err(invalid_params(format!(
                    "unknown J-Quants industry group key: {:?}",
                    params.sector
                )));
            }
            StockGroupSyncSourceCodeLookup::Missing => {
                return Err(internal_error(format!(
                    "J-Quants code for industry group {:?} is not synchronized yet",
                    params.sector
                )));
            }
            StockGroupSyncSourceCodeLookup::Found(code) => code,
            StockGroupSyncSourceCodeLookup::Ambiguous => {
                return Err(internal_error(format!(
                    "J-Quants code for industry group {:?} is ambiguous",
                    params.sector
                )));
            }
        };
        let limit = clamp_limit(params.limit);
        let rows = self
            .dependencies
            .short_ratios
            .read(
                scope,
                ShortRatioQuery {
                    sector33_code,
                    from: params.from,
                    to: params.to,
                    limit,
                },
            )
            .await
            .map_err(|error| {
                tracing::error!(%error, "strategy mcp short ratio query failed");
                internal_error(format!("database error: {error}"))
            })?;

        Ok(ReadSectorShortRatioResult {
            sector: params.sector,
            items: rows.into_iter().map(sector_short_ratio_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::short_ratio::ShortRatioRepository;
    use core_domain::short_ratio::ShortRatio;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use gateway_postgres::entities::{group_axis, stock_group};
    use gateway_postgres::{DatabaseHandle, PostgresShortRatioRepository};

    use super::super::dto::{
        ReadSectorShortRatioParams, ReadSectorShortRatioResult, SectorShortRatioDto,
    };
    use super::super::tests_common::build_server;
    use super::compute_short_ratio;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn ratio(
        date: NaiveDate,
        sector33_code: &str,
        values: Option<(&str, &str, &str)>,
    ) -> ShortRatio {
        let (
            sell_excluding_short_value,
            short_with_restriction_value,
            short_without_restriction_value,
        ) = values.map_or((None, None, None), |(sell, with_r, without_r)| {
            (Some(dec(sell)), Some(dec(with_r)), Some(dec(without_r)))
        });
        ShortRatio {
            date,
            sector33_code: sector33_code.into(),
            sell_excluding_short_value,
            short_with_restriction_value,
            short_without_restriction_value,
        }
    }

    async fn insert_jquants_group(db: &DatabaseHandle, group_key: &str, source_code: Option<&str>) {
        let axis = group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(Uuid::new_v4()),
            key: Set("sample-jquants-axis".into()),
            name: Set("Sample synchronized axis".into()),
            description: Set("Synthetic test axis".into()),
            sync_source: Set(Some("jquants".into())),
        })
        .exec_with_returning(db)
        .await
        .expect("insert group axis");
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(Uuid::new_v4()),
            axis_id: Set(axis.id),
            key: Set(group_key.into()),
            name: Set(group_key.into()),
            description: Set(None),
            sync_source_code: Set(source_code.map(str::to_owned)),
        })
        .exec_without_returning(db)
        .await
        .expect("insert stock group");
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
            expected,
        );
    }

    #[backend_test_macros::database_test]
    async fn rejects_unknown_group_key(db: DatabaseHandle) {
        let error = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "合成業種".into(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect_err("unknown group key should be rejected");

        assert_eq!(
            (error.code, error.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "unknown J-Quants industry group key: \"合成業種\"",
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reports_when_group_code_has_not_been_synchronized(db: DatabaseHandle) {
        insert_jquants_group(&db, "その他", None).await;
        let error = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "その他".into(),
                    from: None,
                    to: None,
                    limit: None,
                },
            )
            .await
            .expect_err("missing synchronized code should be reported");

        assert_eq!(
            (error.code, error.message.as_ref()),
            (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "J-Quants code for industry group \"その他\" is not synchronized yet",
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn reads_sector_rows_and_maps_values_to_result(db: DatabaseHandle) {
        insert_jquants_group(&db, "その他", Some("1234")).await;
        PostgresShortRatioRepository::new(db.clone())
            .upsert(vec![
                ratio(ymd(2025, 1, 5), "1234", Some(("700", "200", "100"))),
                ratio(ymd(2025, 1, 6), "1234", None),
                ratio(ymd(2025, 1, 6), "5678", Some(("100", "50", "50"))),
            ])
            .await
            .expect("seed short ratios");

        let result = build_server(db)
            .read_sector_short_ratio_inner(
                Uuid::new_v4(),
                ReadSectorShortRatioParams {
                    sector: "その他".into(),
                    from: Some(ymd(2025, 1, 5)),
                    to: Some(ymd(2025, 1, 6)),
                    limit: Some(5),
                },
            )
            .await
            .expect("read short ratio");

        assert_eq!(
            result,
            ReadSectorShortRatioResult {
                sector: "その他".into(),
                items: vec![
                    SectorShortRatioDto {
                        date: ymd(2025, 1, 6),
                        sell_excluding_short_value: None,
                        short_with_restriction_value: None,
                        short_without_restriction_value: None,
                        short_ratio: None,
                    },
                    SectorShortRatioDto {
                        date: ymd(2025, 1, 5),
                        sell_excluding_short_value: Some(700.0),
                        short_with_restriction_value: Some(200.0),
                        short_without_restriction_value: Some(100.0),
                        short_ratio: Some(0.3),
                    },
                ],
            },
        );
    }
}
