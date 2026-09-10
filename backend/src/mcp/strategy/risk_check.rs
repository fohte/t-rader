//! 「この銘柄をあと何株買っていいか」を制約ごとに算出する `check_buyable_qty` の inner method 実装。
//!
//! LLM の判断を挟まず、risk_policy に基づく決定論的な計算で上限株数を出す。保有していない
//! 銘柄でも、価格さえ取得できれば現在保有 0 株として計算する。計算に必要な値
//! (価格・投資可能額・セクター) が欠けている制約は、誤った数値を返す代わりに
//! `ConstraintResult::Unavailable` で「計算不能」を明示する。

use std::collections::{BTreeSet, HashMap};

use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::{stock, strategy};
use crate::models::{AccountRiskPolicyData, StrategyRiskPolicyData, parse_risk_policy};
use crate::services::account_risk_policy;
use crate::services::investable_amount;
use crate::services::market_price::fetch_latest_prices;
use crate::services::trades::fetch_summary;

use super::dto::{CheckBuyableQtyParams, CheckBuyableQtyResult, ConstraintResult};
use super::{StrategyServer, app_error_to_mcp, db_error, decimal_to_f64, invalid_params};

/// 日本株の単元株数 (100 株)。上限株数はすべてこの倍数に切り捨てて返す。
const LOT_SIZE: i64 = 100;

impl StrategyServer {
    pub(crate) async fn check_buyable_qty_inner(
        &self,
        strategy_id: Uuid,
        params: CheckBuyableQtyParams,
    ) -> Result<CheckBuyableQtyResult, McpError> {
        let symbol = params.symbol;

        let strategy = strategy::Entity::find_by_id(strategy_id)
            .one(&self.db)
            .await
            .map_err(db_error)?
            .ok_or_else(|| invalid_params(format!("strategy {strategy_id} not found")))?;

        let strategy_risk_policy: StrategyRiskPolicyData =
            parse_risk_policy(strategy.risk_policy).map_err(app_error_to_mcp)?;

        let account_risk_policy_row = account_risk_policy::find_current(&self.db)
            .await
            .map_err(app_error_to_mcp)?;
        let max_sector_ratio = match account_risk_policy_row {
            Some(row) => {
                parse_risk_policy::<AccountRiskPolicyData>(row.risk_policy)
                    .map_err(app_error_to_mcp)?
                    .max_sector_ratio
            }
            None => None,
        };

        let account_summary = fetch_summary(&self.db, None).await.map_err(db_error)?;
        let strategy_summary = fetch_summary(&self.db, Some(strategy_id))
            .await
            .map_err(db_error)?;

        let mut symbols: BTreeSet<String> = account_summary
            .positions
            .iter()
            .map(|p| p.symbol.clone())
            .collect();
        symbols.insert(symbol.clone());
        let symbols: Vec<String> = symbols.into_iter().collect();

        let prices = fetch_latest_prices(&self.db, self.data_provider.as_deref(), &symbols).await;
        let target_price = prices.prices.get(&symbol).copied();

        let current_qty = strategy_summary
            .positions
            .iter()
            .find(|p| p.symbol == symbol)
            .map(|p| p.qty)
            .unwrap_or(Decimal::ZERO);

        let sector_by_symbol = fetch_sector_by_symbol(&self.db, &symbols)
            .await
            .map_err(db_error)?;
        let target_sector = sector_by_symbol.get(&symbol).cloned().flatten();

        let missing_price_symbols: Vec<String> = account_summary
            .positions
            .iter()
            .filter(|p| !prices.prices.contains_key(&p.symbol))
            .map(|p| p.symbol.clone())
            .collect();

        let account_total_value: Decimal = account_summary
            .positions
            .iter()
            .filter_map(|p| prices.prices.get(&p.symbol).map(|price| p.qty * price))
            .sum();

        let sector_value: Decimal = match &target_sector {
            Some(sector_id) => account_summary
                .positions
                .iter()
                .filter(|p| {
                    sector_by_symbol.get(&p.symbol).and_then(|s| s.as_deref())
                        == Some(sector_id.as_str())
                })
                .filter_map(|p| prices.prices.get(&p.symbol).map(|price| p.qty * price))
                .sum(),
            None => Decimal::ZERO,
        };

        let investable_amount_row = investable_amount::find_current(&self.db, strategy_id)
            .await
            .map_err(app_error_to_mcp)?;

        let position_ratio_result = compute_position_ratio_constraint(
            strategy_risk_policy.max_position_ratio,
            target_price,
            current_qty,
            investable_amount_row.as_ref().map(|r| r.amount_jpy),
            &symbol,
        );

        let sector_ratio_result = compute_sector_ratio_constraint(
            max_sector_ratio,
            target_price,
            target_sector.as_deref(),
            sector_value,
            account_total_value,
            &missing_price_symbols,
            &symbol,
        );

        let strategy_cost_basis: Decimal = strategy_summary
            .positions
            .iter()
            .map(|p| p.cost_basis)
            .sum();
        let unused_investable_amount = super::unused_investable_amount(
            investable_amount_row.as_ref().map(|row| row.amount_jpy),
            strategy_summary.realized_pnl,
            strategy_cost_basis,
        );
        let cash_result = compute_cash_constraint(unused_investable_amount, target_price, &symbol);

        let (max_qty, binding_constraint) = combine_constraints([
            ("position_ratio", &position_ratio_result),
            ("sector_ratio", &sector_ratio_result),
            ("cash", &cash_result),
        ]);

        Ok(CheckBuyableQtyResult {
            symbol,
            lot_size: LOT_SIZE,
            current_qty: decimal_to_f64(current_qty),
            current_price: target_price.map(decimal_to_f64),
            priced_at: prices.priced_at,
            max_qty_by_position_ratio: position_ratio_result,
            max_qty_by_sector_ratio: sector_ratio_result,
            max_qty_by_cash: cash_result,
            max_qty,
            binding_constraint,
        })
    }
}

async fn fetch_sector_by_symbol(
    db: &DatabaseConnection,
    symbols: &[String],
) -> Result<HashMap<String, Option<String>>, DbErr> {
    let rows = stock::Entity::find()
        .filter(stock::Column::Id.is_in(symbols.to_vec()))
        .all(db)
        .await?;
    Ok(rows.into_iter().map(|s| (s.id, s.sector_id)).collect())
}

/// 円建ての残り購入余力 (`headroom`) を株数に変換し、単元株に切り捨てる。
fn additional_qty_from_headroom(headroom: Decimal, price: Decimal) -> i64 {
    if headroom <= Decimal::ZERO || price <= Decimal::ZERO {
        return 0;
    }
    let raw = (headroom / price).floor().to_i64().unwrap_or(0);
    floor_to_lot(raw)
}

fn floor_to_lot(qty: i64) -> i64 {
    (qty / LOT_SIZE) * LOT_SIZE
}

fn compute_position_ratio_constraint(
    max_ratio: Option<Decimal>,
    price: Option<Decimal>,
    current_qty: Decimal,
    investable_amount: Option<Decimal>,
    symbol: &str,
) -> ConstraintResult {
    let Some(ratio) = max_ratio else {
        return ConstraintResult::Unlimited;
    };
    let Some(price) = price else {
        return ConstraintResult::Unavailable {
            reason: format!("price unavailable for {symbol}"),
        };
    };
    let Some(investable_amount) = investable_amount else {
        return ConstraintResult::Unavailable {
            reason: "no investable amount recorded for this strategy".to_string(),
        };
    };
    let headroom = ratio * investable_amount - current_qty * price;
    ConstraintResult::Limited {
        max_additional_qty: additional_qty_from_headroom(headroom, price),
    }
}

fn compute_sector_ratio_constraint(
    max_ratio: Option<Decimal>,
    price: Option<Decimal>,
    target_sector: Option<&str>,
    sector_value: Decimal,
    total_value: Decimal,
    missing_price_symbols: &[String],
    symbol: &str,
) -> ConstraintResult {
    let Some(ratio) = max_ratio else {
        return ConstraintResult::Unlimited;
    };
    if target_sector.is_none() {
        return ConstraintResult::Unavailable {
            reason: format!("{symbol} has no sector assigned"),
        };
    }
    let Some(price) = price else {
        return ConstraintResult::Unavailable {
            reason: format!("price unavailable for {symbol}"),
        };
    };
    if !missing_price_symbols.is_empty() {
        return ConstraintResult::Unavailable {
            reason: format!(
                "missing price for held position(s), account-wide total is unreliable: {}",
                missing_price_symbols.join(", ")
            ),
        };
    }

    // 上限比率が 1 (100%) 以上のとき、セクター比率は上限を超え得ないため無制限。
    let denom = price * (Decimal::ONE - ratio);
    if denom <= Decimal::ZERO {
        return ConstraintResult::Unlimited;
    }
    let numerator = ratio * total_value - sector_value;
    let max_additional_qty = if numerator <= Decimal::ZERO {
        0
    } else {
        floor_to_lot((numerator / denom).floor().to_i64().unwrap_or(0))
    };
    ConstraintResult::Limited { max_additional_qty }
}

fn compute_cash_constraint(
    unused_investable_amount: Option<Decimal>,
    price: Option<Decimal>,
    symbol: &str,
) -> ConstraintResult {
    let Some(unused) = unused_investable_amount else {
        return ConstraintResult::Unavailable {
            reason: "no investable amount recorded for this strategy".to_string(),
        };
    };
    let Some(price) = price else {
        return ConstraintResult::Unavailable {
            reason: format!("price unavailable for {symbol}"),
        };
    };
    ConstraintResult::Limited {
        max_additional_qty: additional_qty_from_headroom(unused, price),
    }
}

/// 制約結果を集約する。いずれかが Unavailable なら全体を Unavailable とし、
/// それ以外は最小の Limited を採用する (同値は前方の制約を優先)。
fn combine_constraints(
    constraints: [(&str, &ConstraintResult); 3],
) -> (ConstraintResult, Option<String>) {
    for (name, result) in &constraints {
        if let ConstraintResult::Unavailable { reason } = result {
            return (
                ConstraintResult::Unavailable {
                    reason: format!("{name}: {reason}"),
                },
                None,
            );
        }
    }

    let mut best: Option<(&str, i64)> = None;
    for (name, result) in &constraints {
        if let ConstraintResult::Limited { max_additional_qty } = result {
            best = Some(match best {
                Some((best_name, best_qty)) if best_qty <= *max_additional_qty => {
                    (best_name, best_qty)
                }
                _ => (name, *max_additional_qty),
            });
        }
    }

    match best {
        Some((name, qty)) => (
            ConstraintResult::Limited {
                max_additional_qty: qty,
            },
            Some(name.to_string()),
        ),
        None => (ConstraintResult::Unlimited, None),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::negative_headroom(Decimal::new(-1, 0), Decimal::from(1000), 0)]
    #[case::zero_headroom(Decimal::ZERO, Decimal::from(1000), 0)]
    #[case::zero_price(Decimal::from(1000), Decimal::ZERO, 0)]
    #[case::exact_lot_multiple(Decimal::from(200_000), Decimal::from(1000), 200)]
    #[case::floors_partial_lot(Decimal::from(150_499), Decimal::from(1000), 100)]
    fn additional_qty_from_headroom_cases(
        #[case] headroom: Decimal,
        #[case] price: Decimal,
        #[case] expected: i64,
    ) {
        assert_eq!(additional_qty_from_headroom(headroom, price), expected);
    }

    #[rstest]
    #[case::no_ratio_is_unlimited(
        None,
        Some(Decimal::from(1000)),
        Decimal::ZERO,
        Some(Decimal::from(1_000_000)),
        ConstraintResult::Unlimited
    )]
    #[case::missing_price_is_unavailable(
        Some(Decimal::new(2, 1)),
        None,
        Decimal::ZERO,
        Some(Decimal::from(1_000_000)),
        ConstraintResult::Unavailable { reason: "price unavailable for 7203".to_string() }
    )]
    #[case::missing_investable_amount_is_unavailable(
        Some(Decimal::new(2, 1)),
        Some(Decimal::from(1000)),
        Decimal::ZERO,
        None,
        ConstraintResult::Unavailable { reason: "no investable amount recorded for this strategy".to_string() }
    )]
    #[case::computes_headroom_over_investable_amount(
        Some(Decimal::new(2, 1)),
        Some(Decimal::from(1000)),
        Decimal::ZERO,
        Some(Decimal::from(1_000_000)),
        ConstraintResult::Limited { max_additional_qty: 200 }
    )]
    #[case::subtracts_current_position_value(
        Some(Decimal::new(2, 1)),
        Some(Decimal::from(1000)),
        Decimal::from(100),
        Some(Decimal::from(1_000_000)),
        ConstraintResult::Limited { max_additional_qty: 100 }
    )]
    fn compute_position_ratio_constraint_cases(
        #[case] max_ratio: Option<Decimal>,
        #[case] price: Option<Decimal>,
        #[case] current_qty: Decimal,
        #[case] investable_amount: Option<Decimal>,
        #[case] expected: ConstraintResult,
    ) {
        assert_eq!(
            compute_position_ratio_constraint(
                max_ratio,
                price,
                current_qty,
                investable_amount,
                "7203"
            ),
            expected
        );
    }

    #[rstest]
    #[case::no_ratio_is_unlimited(
        None, Some("transport"), Some(Decimal::from(1000)), Decimal::ZERO, Decimal::ZERO, &[],
        ConstraintResult::Unlimited
    )]
    #[case::no_sector_is_unavailable(
        Some(Decimal::new(2, 1)), None, Some(Decimal::from(1000)), Decimal::ZERO, Decimal::ZERO, &[],
        ConstraintResult::Unavailable { reason: "7203 has no sector assigned".to_string() }
    )]
    #[case::missing_price_is_unavailable(
        Some(Decimal::new(2, 1)), Some("transport"), None, Decimal::ZERO, Decimal::ZERO, &[],
        ConstraintResult::Unavailable { reason: "price unavailable for 7203".to_string() }
    )]
    #[case::ratio_one_is_unlimited(
        Decimal::ONE.into(), Some("transport"), Some(Decimal::from(1000)), Decimal::from(500_000), Decimal::from(1_000_000), &[],
        ConstraintResult::Unlimited
    )]
    fn compute_sector_ratio_constraint_cases(
        #[case] max_ratio: Option<Decimal>,
        #[case] target_sector: Option<&str>,
        #[case] price: Option<Decimal>,
        #[case] sector_value: Decimal,
        #[case] total_value: Decimal,
        #[case] missing_price_symbols: &[String],
        #[case] expected: ConstraintResult,
    ) {
        assert_eq!(
            compute_sector_ratio_constraint(
                max_ratio,
                price,
                target_sector,
                sector_value,
                total_value,
                missing_price_symbols,
                "7203"
            ),
            expected
        );
    }

    #[test]
    fn compute_sector_ratio_constraint_solves_for_max_additional_qty() {
        // sector 時価 200,000 / 口座全体 300,000、上限比率 0.8、価格 1,000
        // => (0.8*300,000 - 200,000) / (1,000*0.2) = 40,000 / 200 = 200 株
        let result = compute_sector_ratio_constraint(
            Some(Decimal::new(8, 1)),
            Some(Decimal::from(1000)),
            Some("transport"),
            Decimal::from(200_000),
            Decimal::from(300_000),
            &[],
            "7203",
        );
        assert_eq!(
            result,
            ConstraintResult::Limited {
                max_additional_qty: 200
            }
        );
    }

    #[rstest]
    #[case::missing_price_symbols_is_unavailable(
        Some(Decimal::new(2, 1)),
        Some("transport"),
        Some(Decimal::from(1000)),
        Decimal::ZERO,
        Decimal::ZERO,
        &["6758".to_string()],
        ConstraintResult::Unavailable {
            reason: "missing price for held position(s), account-wide total is unreliable: 6758".to_string()
        }
    )]
    fn compute_sector_ratio_constraint_missing_prices(
        #[case] max_ratio: Option<Decimal>,
        #[case] target_sector: Option<&str>,
        #[case] price: Option<Decimal>,
        #[case] sector_value: Decimal,
        #[case] total_value: Decimal,
        #[case] missing_price_symbols: &[String],
        #[case] expected: ConstraintResult,
    ) {
        assert_eq!(
            compute_sector_ratio_constraint(
                max_ratio,
                price,
                target_sector,
                sector_value,
                total_value,
                missing_price_symbols,
                "7203"
            ),
            expected
        );
    }

    #[rstest]
    #[case::missing_investable_amount_is_unavailable(
        None, Some(Decimal::from(1000)),
        ConstraintResult::Unavailable { reason: "no investable amount recorded for this strategy".to_string() }
    )]
    #[case::missing_price_is_unavailable(
        Some(Decimal::from(1_000_000)), None,
        ConstraintResult::Unavailable { reason: "price unavailable for 7203".to_string() }
    )]
    #[case::computes_from_unused_amount(
        Some(Decimal::from(1_000_000)), Some(Decimal::from(1000)),
        ConstraintResult::Limited { max_additional_qty: 1000 }
    )]
    #[case::negative_unused_amount_floors_to_zero(
        Some(Decimal::from(-1)), Some(Decimal::from(1000)),
        ConstraintResult::Limited { max_additional_qty: 0 }
    )]
    fn compute_cash_constraint_cases(
        #[case] unused_investable_amount: Option<Decimal>,
        #[case] price: Option<Decimal>,
        #[case] expected: ConstraintResult,
    ) {
        assert_eq!(
            compute_cash_constraint(unused_investable_amount, price, "7203"),
            expected
        );
    }

    #[rstest]
    #[case::all_unlimited_is_unlimited(
        ConstraintResult::Unlimited,
        ConstraintResult::Unlimited,
        ConstraintResult::Unlimited,
        ConstraintResult::Unlimited,
        None
    )]
    #[case::any_unavailable_poisons_overall(
        ConstraintResult::Limited { max_additional_qty: 100 },
        ConstraintResult::Unavailable { reason: "x".to_string() },
        ConstraintResult::Limited { max_additional_qty: 50 },
        ConstraintResult::Unavailable { reason: "sector_ratio: x".to_string() },
        None
    )]
    #[case::picks_minimum_limited(
        ConstraintResult::Limited { max_additional_qty: 1500 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Limited { max_additional_qty: 1800 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        Some("sector_ratio")
    )]
    #[case::ties_prefer_earlier_constraint(
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Unlimited,
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        Some("position_ratio")
    )]
    fn combine_constraints_cases(
        #[case] position_ratio: ConstraintResult,
        #[case] sector_ratio: ConstraintResult,
        #[case] cash: ConstraintResult,
        #[case] expected_max_qty: ConstraintResult,
        #[case] expected_binding: Option<&str>,
    ) {
        let (max_qty, binding_constraint) = combine_constraints([
            ("position_ratio", &position_ratio),
            ("sector_ratio", &sector_ratio),
            ("cash", &cash),
        ]);
        assert_eq!(max_qty, expected_max_qty);
        assert_eq!(binding_constraint, expected_binding.map(str::to_string));
    }
}

#[cfg(test)]
mod integration_tests {
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseConnection, EntityTrait};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{instruments, sector, stock, strategy, trade};
    use crate::models::{Bar, Timeframe};
    use crate::repositories::bars::upsert_bars;
    use crate::services::{account_risk_policy, investable_amount};
    use crate::testing::create_test_db;

    use super::super::dto::{CheckBuyableQtyParams, CheckBuyableQtyResult, ConstraintResult};
    use super::super::tests_common::{build_server, insert_strategy};

    async fn seed_trade(
        db: &DatabaseConnection,
        strategy_id: Uuid,
        symbol: &str,
        qty: i64,
        price: i64,
    ) {
        trade::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            symbol: Set(symbol.to_string()),
            side: Set("buy".to_string()),
            qty: Set(Decimal::from(qty)),
            price: Set(Decimal::from(price)),
            fee: Set(Decimal::ZERO),
            date: Set(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed trade");
    }

    async fn seed_bar(db: &DatabaseConnection, symbol: &str, close: i64) {
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(symbol.to_string()),
            name: Set(symbol.to_string()),
            market: Set("TSE".to_string()),
            sector: Set(None),
        })
        .exec(db)
        .await
        .expect("insert test instrument");

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date");
        let timestamp = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("time"));
        upsert_bars(
            db,
            vec![Bar {
                instrument_id: symbol.to_string(),
                timeframe: Timeframe::Daily,
                timestamp,
                open: Decimal::from(close),
                high: Decimal::from(close),
                low: Decimal::from(close),
                close: Decimal::from(close),
                volume: 1000,
            }],
        )
        .await
        .expect("seed bar");
    }

    async fn insert_stock(db: &DatabaseConnection, symbol: &str, sector_id: Option<&str>) {
        if let Some(sector_id) = sector_id {
            sector::Entity::insert(sector::ActiveModel {
                id: Set(sector_id.to_string()),
                name: Set(sector_id.to_string()),
            })
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(sector::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await
            .expect("insert test sector");
        }
        stock::ActiveModel {
            id: Set(symbol.to_string()),
            name: Set(symbol.to_string()),
            market: Set(None),
            sector_id: Set(sector_id.map(str::to_string)),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test stock");
    }

    async fn set_max_position_ratio(db: &DatabaseConnection, strategy_id: Uuid, ratio: &str) {
        let mut active: strategy::ActiveModel = strategy::Entity::find_by_id(strategy_id)
            .one(db)
            .await
            .expect("query ok")
            .expect("strategy exists")
            .into();
        active.risk_policy = Set(serde_json::json!({ "max_position_ratio": ratio }));
        active.update(db).await.expect("set max_position_ratio");
    }

    async fn set_max_sector_ratio(db: &DatabaseConnection, ratio: &str) {
        account_risk_policy::save(db, serde_json::json!({ "max_sector_ratio": ratio }))
            .await
            .expect("set max_sector_ratio");
    }

    async fn record_investable_amount(db: &DatabaseConnection, strategy_id: Uuid, amount: i64) {
        investable_amount::record(
            db,
            strategy_id,
            Decimal::from(amount),
            Utc::now().fixed_offset() - chrono::Duration::days(1),
        )
        .await
        .expect("record investable amount");
    }

    #[sqlx::test(migrations = false)]
    async fn defaults_to_cash_constraint_when_no_risk_policy_is_configured(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        seed_bar(&db, "7203", 1000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_position_ratio: ConstraintResult::Unlimited,
                max_qty_by_sector_ratio: ConstraintResult::Unlimited,
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 1000
                },
                max_qty: ConstraintResult::Limited {
                    max_additional_qty: 1000
                },
                binding_constraint: Some("cash".to_string()),
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn position_ratio_binds_when_stricter_than_cash(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        set_max_position_ratio(&db, strategy_id, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        seed_bar(&db, "7203", 1000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_position_ratio: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                max_qty_by_sector_ratio: ConstraintResult::Unlimited,
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 1000
                },
                max_qty: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                binding_constraint: Some("position_ratio".to_string()),
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn sector_ratio_binds_across_strategies(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        insert_stock(&db, "7203", Some("transport")).await;
        insert_stock(&db, "7267", Some("transport")).await;
        insert_stock(&db, "6758", Some("tech")).await;
        seed_trade(&db, strategy_a, "7203", 100, 1000).await;
        seed_trade(&db, strategy_b, "7267", 200, 500).await;
        seed_trade(&db, strategy_b, "6758", 50, 2000).await;
        seed_bar(&db, "7203", 1000).await;
        seed_bar(&db, "7267", 500).await;
        seed_bar(&db, "6758", 2000).await;
        set_max_sector_ratio(&db, "0.8").await;
        record_investable_amount(&db, strategy_a, 100_000_000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_a,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 100.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_position_ratio: ConstraintResult::Unlimited,
                max_qty_by_sector_ratio: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 99900
                },
                max_qty: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                binding_constraint: Some("sector_ratio".to_string()),
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn all_constraints_become_unavailable_when_target_price_is_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        set_max_position_ratio(&db, strategy_id, "0.2").await;
        set_max_sector_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        // "7203" の bar を意図的に seed しない (価格取得不可を再現)
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: None,
                priced_at: None,
                max_qty_by_position_ratio: ConstraintResult::Unavailable {
                    reason: "price unavailable for 7203".to_string()
                },
                max_qty_by_sector_ratio: ConstraintResult::Unavailable {
                    reason: "7203 has no sector assigned".to_string()
                },
                max_qty_by_cash: ConstraintResult::Unavailable {
                    reason: "price unavailable for 7203".to_string()
                },
                max_qty: ConstraintResult::Unavailable {
                    reason: "position_ratio: price unavailable for 7203".to_string()
                },
                binding_constraint: None,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn sector_ratio_is_unavailable_when_target_has_no_sector(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        set_max_sector_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        seed_bar(&db, "7203", 1000).await;
        // "7203" は stock 行を作らない (sector_id 不明を再現)
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_position_ratio: ConstraintResult::Unlimited,
                max_qty_by_sector_ratio: ConstraintResult::Unavailable {
                    reason: "7203 has no sector assigned".to_string()
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 1000
                },
                max_qty: ConstraintResult::Unavailable {
                    reason: "sector_ratio: 7203 has no sector assigned".to_string()
                },
                binding_constraint: None,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn sector_ratio_is_unavailable_when_a_held_position_price_is_missing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "a").await;
        insert_stock(&db, "7203", Some("transport")).await;
        insert_stock(&db, "9999", Some("tech")).await;
        seed_trade(&db, strategy_id, "7203", 100, 1000).await;
        seed_trade(&db, strategy_id, "9999", 10, 100).await;
        seed_bar(&db, "7203", 1000).await;
        // "9999" の bar は意図的に seed しない
        set_max_sector_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "7203".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "7203".to_string(),
                lot_size: 100,
                current_qty: 100.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_position_ratio: ConstraintResult::Unlimited,
                max_qty_by_sector_ratio: ConstraintResult::Unavailable {
                    reason:
                        "missing price for held position(s), account-wide total is unreliable: 9999"
                            .to_string()
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 800
                },
                max_qty: ConstraintResult::Unavailable {
                    reason:
                        "sector_ratio: missing price for held position(s), account-wide total is unreliable: 9999"
                            .to_string()
                },
                binding_constraint: None,
            }
        );
    }
}
