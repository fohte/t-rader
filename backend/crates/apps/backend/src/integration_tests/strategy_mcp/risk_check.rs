#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use core_domain::bar::{Bar, Timeframe};

    use super::super::dto::{CheckBuyableQtyParams, CheckBuyableQtyResult, ConstraintResult};
    use super::super::tests_common::{build_server, insert_strategy};
    use gateway_postgres::entities::{
        account_risk_policy, group_axis, instruments, stock, stock_group, stock_group_member,
        strategy_investable_amount, trade,
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
            derive_from: Set(None),
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
            code: Set(None),
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
        account_risk_policy::ActiveModel {
            id: Set(1),
            risk_policy: Set(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": ratio }]
            })),
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("set max_group_ratios");
    }

    async fn record_investable_amount(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        amount: i64,
    ) {
        strategy_investable_amount::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            amount_jpy: Set(Decimal::from(amount)),
            effective_at: Set(Utc::now().fixed_offset() - chrono::Duration::days(1)),
            created_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert investable amount");
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
            .check_buyable_qty(
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
            .check_buyable_qty(
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
            .check_buyable_qty(
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
            .check_buyable_qty(
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
            .check_buyable_qty(
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
