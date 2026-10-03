//! 「この銘柄をあと何株買っていいか」を制約ごとに算出する `check_buyable_qty` の inner method 実装。
//!
//! LLM の判断を挟まず、risk_policy に基づく決定論的な計算で上限株数を出す。保有していない
//! 銘柄でも、価格さえ取得できれば現在保有 0 株として計算する。計算に必要な値
//! (価格・投資可能額・分類軸グループ) が欠けている制約は、誤った数値を返す代わりに
//! `ConstraintResult::Unavailable` で「計算不能」を明示する。

use std::collections::{BTreeSet, HashMap};

use core_application::account_risk_policy::{
    AccountRiskPolicyData, AccountRiskPolicyRepositoryError, parse_risk_policy,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::dto::{CheckBuyableQtyParams, CheckBuyableQtyResult, ConstraintResult};
use super::{
    StrategyServer, decimal_to_f64, internal_failure, persistence_error_to_mcp,
    strategy_use_case_error_to_mcp, trade_error,
};

/// 日本株の単元株数 (100 株)。上限株数はすべてこの倍数に切り捨てて返す。
const LOT_SIZE: i64 = 100;

fn account_risk_policy_error_to_mcp(error: AccountRiskPolicyRepositoryError) -> McpError {
    match error {
        AccountRiskPolicyRepositoryError::Database(error) => persistence_error_to_mcp(error),
    }
}

fn parse_account_risk_policy(value: serde_json::Value) -> Result<AccountRiskPolicyData, McpError> {
    parse_risk_policy(value).map_err(|error| internal_failure(&error.to_string()))
}

impl StrategyServer {
    pub(crate) async fn check_buyable_qty_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: CheckBuyableQtyParams,
    ) -> Result<CheckBuyableQtyResult, McpError> {
        let scope = scope.into();
        let strategy_id = scope.id();
        let symbol = params.symbol;

        let account_risk_policy = self
            .dependencies
            .account_risk_policies
            .find_current()
            .await
            .map_err(account_risk_policy_error_to_mcp)?;
        let max_group_ratios: Vec<(String, Decimal)> = match account_risk_policy {
            Some(risk_policy) => parse_account_risk_policy(risk_policy)?
                .max_group_ratios
                .into_iter()
                .map(|group_ratio| (group_ratio.axis, group_ratio.ratio))
                .collect(),
            None => Vec::new(),
        };

        let account_summary = self
            .dependencies
            .trades
            .summary(None)
            .await
            .map_err(trade_error)?;
        let strategy_summary = self
            .dependencies
            .trades
            .summary(Some(strategy_id))
            .await
            .map_err(trade_error)?;

        let mut symbols: BTreeSet<String> = account_summary
            .positions
            .iter()
            .map(|p| p.symbol.clone())
            .collect();
        symbols.insert(symbol.clone());
        let symbols: Vec<String> = symbols.into_iter().collect();

        let prices = self
            .dependencies
            .bars
            .fetch_latest_prices(self.dependencies.daily_bar_source.as_deref(), &symbols)
            .await;
        let target_price = prices.prices.get(&symbol).copied();

        let current_qty = strategy_summary
            .positions
            .iter()
            .find(|p| p.symbol == symbol)
            .map(|p| p.qty)
            .unwrap_or(Decimal::ZERO);

        let axis_keys: Vec<String> = max_group_ratios
            .iter()
            .map(|(axis, _)| axis.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let memberships = self
            .dependencies
            .stock_groups
            .list_memberships(&symbols, &axis_keys)
            .await
            .map_err(super::stock_groups::stock_group_error)?;
        let mut groups_by_axis_and_symbol: HashMap<(String, String), BTreeSet<String>> =
            HashMap::new();
        for membership in memberships {
            groups_by_axis_and_symbol
                .entry((membership.axis_key, membership.stock_id))
                .or_default()
                .insert(membership.group_key);
        }

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

        let mut value_by_axis_and_group: HashMap<(String, String), Decimal> = HashMap::new();
        for position in &account_summary.positions {
            let Some(price) = prices.prices.get(&position.symbol) else {
                continue;
            };
            for axis_key in &axis_keys {
                if let Some(group_keys) =
                    groups_by_axis_and_symbol.get(&(axis_key.clone(), position.symbol.clone()))
                {
                    for group_key in group_keys {
                        *value_by_axis_and_group
                            .entry((axis_key.clone(), group_key.clone()))
                            .or_default() += position.qty * price;
                    }
                }
            }
        }

        let investable_amount_row = self
            .dependencies
            .strategies
            .current_investable_amount(scope)
            .await
            .map_err(strategy_use_case_error_to_mcp)?;

        let group_ratio_result = compute_group_ratio_constraint(
            &max_group_ratios,
            target_price,
            &groups_by_axis_and_symbol,
            &value_by_axis_and_group,
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
            ("group_ratios", &group_ratio_result),
            ("cash", &cash_result),
        ]);

        Ok(CheckBuyableQtyResult {
            symbol,
            lot_size: LOT_SIZE,
            current_qty: decimal_to_f64(current_qty),
            current_price: target_price.map(decimal_to_f64),
            priced_at: prices.priced_at,
            max_qty_by_group_ratios: group_ratio_result,
            max_qty_by_cash: cash_result,
            max_qty,
            binding_constraint,
        })
    }
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

fn compute_group_ratio_constraint(
    max_group_ratios: &[(String, Decimal)],
    price: Option<Decimal>,
    groups_by_axis_and_symbol: &HashMap<(String, String), BTreeSet<String>>,
    value_by_axis_and_group: &HashMap<(String, String), Decimal>,
    total_value: Decimal,
    missing_price_symbols: &[String],
    symbol: &str,
) -> ConstraintResult {
    if max_group_ratios.is_empty() {
        return ConstraintResult::Unlimited;
    }

    let mut target_groups = Vec::with_capacity(max_group_ratios.len());
    for (axis_key, ratio) in max_group_ratios {
        let Some(group_keys) = groups_by_axis_and_symbol
            .get(&(axis_key.clone(), symbol.to_owned()))
            .filter(|groups| !groups.is_empty())
        else {
            return ConstraintResult::Unavailable {
                reason: format!("{symbol} has no group assigned for axis {axis_key}"),
            };
        };
        target_groups.push((axis_key, ratio, group_keys));
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

    let mut max_additional_qty: Option<i64> = None;
    for (axis_key, ratio, group_keys) in target_groups {
        let denominator = price * (Decimal::ONE - ratio);
        if denominator <= Decimal::ZERO {
            continue;
        }
        for group_key in group_keys {
            let group_value = value_by_axis_and_group
                .get(&(axis_key.clone(), group_key.clone()))
                .copied()
                .unwrap_or(Decimal::ZERO);
            let numerator = ratio * total_value - group_value;
            let group_max = if numerator <= Decimal::ZERO {
                0
            } else {
                floor_to_lot((numerator / denominator).floor().to_i64().unwrap_or(0))
            };
            max_additional_qty =
                Some(max_additional_qty.map_or(group_max, |current| current.min(group_max)));
        }
    }

    max_additional_qty.map_or(ConstraintResult::Unlimited, |max_additional_qty| {
        ConstraintResult::Limited { max_additional_qty }
    })
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
    constraints: [(&str, &ConstraintResult); 2],
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

    #[test]
    fn malformed_risk_policy_maps_to_an_internal_error() {
        assert_eq!(
            parse_account_risk_policy(serde_json::json!(true)),
            Err(rmcp::ErrorData::internal_error(
                "database error: invalid risk_policy: invalid type: boolean `true`, expected struct AccountRiskPolicyData",
                None,
            ))
        );
    }

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
    #[case::no_limits_is_unlimited(
        Vec::new(), HashMap::new(), HashMap::new(), Some(Decimal::from(1000)), &[],
        ConstraintResult::Unlimited
    )]
    #[case::target_without_axis_membership_is_unavailable(
        vec![("sample-axis".to_string(), Decimal::new(2, 1))], HashMap::new(), HashMap::new(),
        Some(Decimal::from(1000)), &[],
        ConstraintResult::Unavailable { reason: "demo-stock has no group assigned for axis sample-axis".to_string() }
    )]
    #[case::missing_price_is_unavailable(
        vec![("sample-axis".to_string(), Decimal::new(2, 1))],
        group_memberships(&[("sample-axis", "sample-group", "demo-stock")]), HashMap::new(), None, &[],
        ConstraintResult::Unavailable { reason: "price unavailable for demo-stock".to_string() }
    )]
    #[case::ratio_one_is_unlimited(
        vec![("sample-axis".to_string(), Decimal::ONE)],
        group_memberships(&[("sample-axis", "sample-group", "demo-stock")]), HashMap::new(),
        Some(Decimal::from(1000)), &[], ConstraintResult::Unlimited
    )]
    #[case::missing_price_for_a_holding_is_unavailable(
        vec![("sample-axis".to_string(), Decimal::new(2, 1))],
        group_memberships(&[("sample-axis", "sample-group", "demo-stock")]), HashMap::new(),
        Some(Decimal::from(1000)), &["demo-unpriced".to_string()],
        ConstraintResult::Unavailable {
            reason: "missing price for held position(s), account-wide total is unreliable: demo-unpriced".to_string()
        }
    )]
    fn compute_group_ratio_constraint_cases(
        #[case] max_group_ratios: Vec<(String, Decimal)>,
        #[case] groups_by_axis_and_symbol: HashMap<(String, String), BTreeSet<String>>,
        #[case] value_by_axis_and_group: HashMap<(String, String), Decimal>,
        #[case] price: Option<Decimal>,
        #[case] missing_price_symbols: &[String],
        #[case] expected: ConstraintResult,
    ) {
        assert_eq!(
            compute_group_ratio_constraint(
                &max_group_ratios,
                price,
                &groups_by_axis_and_symbol,
                &value_by_axis_and_group,
                Decimal::from(1_000_000),
                missing_price_symbols,
                "demo-stock"
            ),
            expected
        );
    }

    #[test]
    fn limits_against_every_target_group_on_each_configured_axis() {
        let memberships = group_memberships(&[
            ("sample-axis", "sample-group-a", "demo-stock"),
            ("sample-axis", "sample-group-b", "demo-stock"),
            ("other-axis", "other-group", "demo-stock"),
        ]);
        let values = HashMap::from([
            (
                ("sample-axis".to_string(), "sample-group-a".to_string()),
                Decimal::from(200_000),
            ),
            (
                ("sample-axis".to_string(), "sample-group-b".to_string()),
                Decimal::from(750_000),
            ),
            (
                ("other-axis".to_string(), "other-group".to_string()),
                Decimal::from(350_000),
            ),
        ]);

        assert_eq!(
            compute_group_ratio_constraint(
                &[
                    ("sample-axis".to_string(), Decimal::new(8, 1)),
                    ("other-axis".to_string(), Decimal::new(5, 1)),
                ],
                Some(Decimal::from(1000)),
                &memberships,
                &values,
                Decimal::from(1_000_000),
                &[],
                "demo-stock",
            ),
            ConstraintResult::Limited {
                max_additional_qty: 200,
            }
        );
    }

    fn group_memberships(
        memberships: &[(&str, &str, &str)],
    ) -> HashMap<(String, String), BTreeSet<String>> {
        let mut grouped = HashMap::new();
        for (axis_key, group_key, stock_id) in memberships {
            grouped
                .entry((axis_key.to_string(), stock_id.to_string()))
                .or_insert_with(BTreeSet::new)
                .insert(group_key.to_string());
        }
        grouped
    }

    #[rstest]
    #[case::missing_investable_amount_is_unavailable(
        None, Some(Decimal::from(1000)),
        ConstraintResult::Unavailable { reason: "no investable amount recorded for this strategy".to_string() }
    )]
    #[case::missing_price_is_unavailable(
        Some(Decimal::from(1_000_000)), None,
        ConstraintResult::Unavailable { reason: "price unavailable for demo-stock".to_string() }
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
            compute_cash_constraint(unused_investable_amount, price, "demo-stock"),
            expected
        );
    }

    #[rstest]
    #[case::all_unlimited_is_unlimited(
        ConstraintResult::Unlimited,
        ConstraintResult::Unlimited,
        ConstraintResult::Unlimited,
        None
    )]
    #[case::any_unavailable_poisons_overall(
        ConstraintResult::Unavailable { reason: "x".to_string() },
        ConstraintResult::Limited { max_additional_qty: 50 },
        ConstraintResult::Unavailable { reason: "group_ratios: x".to_string() },
        None
    )]
    #[case::picks_minimum_limited(
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Limited { max_additional_qty: 1800 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        Some("group_ratios")
    )]
    #[case::ties_prefer_earlier_constraint(
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        ConstraintResult::Limited { max_additional_qty: 1000 },
        Some("group_ratios")
    )]
    fn combine_constraints_cases(
        #[case] group_ratios: ConstraintResult,
        #[case] cash: ConstraintResult,
        #[case] expected_max_qty: ConstraintResult,
        #[case] expected_binding: Option<&str>,
    ) {
        let (max_qty, binding_constraint) =
            combine_constraints([("group_ratios", &group_ratios), ("cash", &cash)]);
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
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use core_domain::bar::{Bar, Timeframe};

    use super::super::dto::{CheckBuyableQtyParams, CheckBuyableQtyResult, ConstraintResult};
    use super::super::tests_common::{build_server, insert_strategy};
    use core_application::change_history::Actor;
    use core_application::strategy_scope::StrategyScope;
    use gateway_postgres::entities::{
        group_axis, instruments, stock, stock_group, stock_group_member, trade,
    };
    use gateway_postgres::repositories::bars::upsert_bars;

    async fn seed_trade(
        db: &impl sea_orm::ConnectionTrait,
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

    async fn seed_bar(db: &impl sea_orm::ConnectionTrait, symbol: &str, close: i64) {
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

    async fn insert_stock(db: &impl sea_orm::ConnectionTrait, symbol: &str) {
        stock::ActiveModel {
            id: Set(symbol.to_string()),
            name: Set(symbol.to_string()),
            market: Set(None),
            product_category: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test stock");
    }

    async fn insert_group_axis(db: &impl sea_orm::ConnectionTrait) -> Uuid {
        let id = Uuid::new_v4();
        group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(id),
            key: Set("sample-axis".to_string()),
            name: Set("Sample axis".to_string()),
            description: Set("Sample axis for tests".to_string()),
            sync_source: Set(None),
        })
        .exec(db)
        .await
        .expect("insert test group axis");
        id
    }

    async fn insert_stock_group(
        db: &impl sea_orm::ConnectionTrait,
        axis_id: Uuid,
        key: &str,
        stock_ids: &[&str],
    ) {
        let group_id = Uuid::new_v4();
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set(key.to_string()),
            name: Set("Sample group".to_string()),
            description: Set(None),
            sync_source_code: Set(None),
        })
        .exec(db)
        .await
        .expect("insert test stock group");
        for stock_id in stock_ids {
            stock_group_member::Entity::insert(stock_group_member::ActiveModel {
                stock_id: Set((*stock_id).to_string()),
                group_id: Set(group_id),
                created_at: NotSet,
            })
            .exec(db)
            .await
            .expect("insert test stock group member");
        }
    }

    async fn set_max_group_ratio(db: &gateway_postgres::DatabaseHandle, ratio: &str) {
        let use_cases = crate::services::use_cases::build_use_cases(db.clone());
        use_cases
            .account_risk_policies()
            .save(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": ratio }]
            }))
            .await
            .expect("set max_group_ratios");
    }

    async fn record_investable_amount(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        amount: i64,
    ) {
        crate::services::use_cases::build_use_cases(db.clone())
            .strategies()
            .record_investable_amount(
                Actor::Human,
                StrategyScope::from(strategy_id),
                Decimal::from(amount),
                Utc::now().fixed_offset() - chrono::Duration::days(1),
            )
            .await
            .expect("record investable amount");
    }

    #[backend_test_macros::database_test]
    async fn defaults_to_cash_constraint_when_no_risk_policy_is_configured(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        seed_bar(&db, "demo-stock", 1000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "demo-stock".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "demo-stock".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_group_ratios: ConstraintResult::Unlimited,
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

    #[backend_test_macros::database_test]
    async fn group_ratio_binds_across_strategies(db: gateway_postgres::DatabaseHandle) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        insert_stock(&db, "demo-stock").await;
        insert_stock(&db, "demo-peer").await;
        insert_stock(&db, "demo-other").await;
        let axis_id = insert_group_axis(&db).await;
        insert_stock_group(&db, axis_id, "sample-group", &["demo-stock", "demo-peer"]).await;
        seed_trade(&db, strategy_a, "demo-stock", 100, 1000).await;
        seed_trade(&db, strategy_b, "demo-peer", 200, 500).await;
        seed_trade(&db, strategy_b, "demo-other", 50, 2000).await;
        seed_bar(&db, "demo-stock", 1000).await;
        seed_bar(&db, "demo-peer", 500).await;
        seed_bar(&db, "demo-other", 2000).await;
        set_max_group_ratio(&db, "0.8").await;
        record_investable_amount(&db, strategy_a, 100_000_000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_a,
                CheckBuyableQtyParams {
                    symbol: "demo-stock".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "demo-stock".to_string(),
                lot_size: 100,
                current_qty: 100.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_group_ratios: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 99900
                },
                max_qty: ConstraintResult::Limited {
                    max_additional_qty: 200
                },
                binding_constraint: Some("group_ratios".to_string()),
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn all_constraints_become_unavailable_when_target_price_is_missing(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        let axis_id = insert_group_axis(&db).await;
        insert_stock(&db, "demo-stock").await;
        insert_stock_group(&db, axis_id, "sample-group", &["demo-stock"]).await;
        set_max_group_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        // 対象銘柄の bar を意図的に seed しない (価格取得不可を再現)
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "demo-stock".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "demo-stock".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: None,
                priced_at: None,
                max_qty_by_group_ratios: ConstraintResult::Unavailable {
                    reason: "price unavailable for demo-stock".to_string()
                },
                max_qty_by_cash: ConstraintResult::Unavailable {
                    reason: "price unavailable for demo-stock".to_string()
                },
                max_qty: ConstraintResult::Unavailable {
                    reason: "group_ratios: price unavailable for demo-stock".to_string()
                },
                binding_constraint: None,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn group_ratio_is_unavailable_when_target_has_no_group_for_axis(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_group_axis(&db).await;
        set_max_group_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        seed_bar(&db, "demo-stock", 1000).await;
        // 対象銘柄に分類軸の所属グループを設定しない。
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "demo-stock".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "demo-stock".to_string(),
                lot_size: 100,
                current_qty: 0.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_group_ratios: ConstraintResult::Unavailable {
                    reason: "demo-stock has no group assigned for axis sample-axis".to_string()
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 1000
                },
                max_qty: ConstraintResult::Unavailable {
                    reason: "group_ratios: demo-stock has no group assigned for axis sample-axis"
                        .to_string()
                },
                binding_constraint: None,
            }
        );
    }

    #[backend_test_macros::database_test]
    async fn group_ratio_is_unavailable_when_a_held_position_price_is_missing(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "a").await;
        insert_stock(&db, "demo-stock").await;
        insert_stock(&db, "demo-unpriced").await;
        let axis_id = insert_group_axis(&db).await;
        insert_stock_group(&db, axis_id, "sample-group", &["demo-stock"]).await;
        seed_trade(&db, strategy_id, "demo-stock", 100, 1000).await;
        seed_trade(&db, strategy_id, "demo-unpriced", 10, 100).await;
        seed_bar(&db, "demo-stock", 1000).await;
        // もう一方の保有銘柄の bar は意図的に seed しない
        set_max_group_ratio(&db, "0.2").await;
        record_investable_amount(&db, strategy_id, 1_000_000).await;
        let server = build_server(db);

        let result = server
            .check_buyable_qty_inner(
                strategy_id,
                CheckBuyableQtyParams {
                    symbol: "demo-stock".to_string(),
                },
            )
            .await
            .expect("check_buyable_qty");

        assert_eq!(
            result,
            CheckBuyableQtyResult {
                symbol: "demo-stock".to_string(),
                lot_size: 100,
                current_qty: 100.0,
                current_price: Some(1000.0),
                priced_at: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date")),
                max_qty_by_group_ratios: ConstraintResult::Unavailable {
                    reason:
                        "missing price for held position(s), account-wide total is unreliable: demo-unpriced"
                            .to_string()
                },
                max_qty_by_cash: ConstraintResult::Limited {
                    max_additional_qty: 800
                },
                max_qty: ConstraintResult::Unavailable {
                    reason:
                        "group_ratios: missing price for held position(s), account-wide total is unreliable: demo-unpriced"
                            .to_string()
                },
                binding_constraint: None,
            }
        );
    }
}
