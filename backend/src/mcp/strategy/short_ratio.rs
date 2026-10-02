//! 戦略実行 MCP の `read_sector_short_ratio` tool。`short_ratio` (J-Quants
//! `/markets/short-ratio` の業種別空売り比率) を業種名・期間で読む。
//!
//! tool の入力は `sync_source = 'jquants'` の分類軸にあるグループ key で受け取る。
//! `short_ratio.sector33_code` は市場データ側の 33 業種コードのため、グループ key から
//! 対応するコードを引く表をここに持つ。
//! 空売り比率の定義 (空売り (価格規制あり+なし) の売買代金 / 実注文と空売りを合わせた
//! 売買代金) は JPX の空売り集計公表ページに基づく。戦略に属さない市場データのため
//! `search_refs` / `search_news` 同様 `x-strategy-id` を検索条件には使わない。

use core_application::short_ratio::ShortRatioQuery;
use core_application::strategy_scope::StrategyScope;
use core_domain::short_ratio::ShortRatio;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;

use super::dto::{ReadSectorShortRatioParams, ReadSectorShortRatioResult, SectorShortRatioDto};
use super::{StrategyServer, clamp_limit, decimal_to_f64, internal_error, invalid_params};

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

fn sector33_code_for_group_key(group_key: &str) -> Option<&'static str> {
    SECTOR33_CODES
        .iter()
        .find(|(name, _)| *name == group_key)
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
        let sector33_code = sector33_code_for_group_key(&params.sector).ok_or_else(|| {
            invalid_params(format!(
                "unknown J-Quants industry group key: {:?}",
                params.sector
            ))
        })?;
        let limit = clamp_limit(params.limit);
        let rows = self
            .dependencies
            .short_ratios
            .read(
                scope,
                ShortRatioQuery {
                    sector33_code: sector33_code.to_string(),
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
    use super::{compute_short_ratio, sector33_code_for_group_key};
    use rstest::rstest;
    use rust_decimal::Decimal;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[rstest]
    #[case::plain_name("輸送用機器", Some("3700"))]
    #[case::halfwidth_dot_name("石油･石炭製品", Some("3300"))]
    #[case::catch_all("その他", Some("9999"))]
    #[case::unknown("合成業種", None)]
    fn sector33_code_for_group_key_cases(#[case] group_key: &str, #[case] expected: Option<&str>) {
        assert_eq!(sector33_code_for_group_key(group_key), expected);
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
}
