#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use uuid::Uuid;

    use super::super::dto::{ReadTradesParams, ReadTradesResult, TradeDto, TradeNoteReferenceDto};
    use super::super::tests_common::{build_server, insert_strategy};
    use gateway_postgres::entities::{trade, trade_note};

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    async fn seed_trade(
        db: &impl sea_orm::ConnectionTrait,
        strategy_id: Uuid,
        symbol: &str,
        side: &str,
        qty: i64,
        price: i64,
        date: NaiveDate,
    ) -> Uuid {
        let id = Uuid::new_v4();
        trade::ActiveModel {
            id: Set(id),
            strategy_id: Set(strategy_id),
            symbol: Set(symbol.to_string()),
            side: Set(side.to_string()),
            qty: Set(Decimal::from(qty)),
            price: Set(Decimal::from(price)),
            fee: Set(Decimal::ZERO),
            date: Set(date),
            source: Set("manual".into()),
            note: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed trade");
        id
    }

    async fn seed_trade_note(
        db: &impl sea_orm::ConnectionTrait,
        trade_id: Uuid,
        note_id: Uuid,
        note_version_id: Uuid,
        created_at: chrono::DateTime<chrono::FixedOffset>,
    ) {
        trade_note::ActiveModel {
            trade_id: Set(trade_id),
            note_id: Set(note_id),
            note_version_id: Set(note_version_id),
            created_at: Set(created_at),
        }
        .insert(db)
        .await
        .expect("link trade note");
    }

    #[backend_test_macros::database_test]
    async fn read_trades_returns_full_shape_across_strategies(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let trade_a = seed_trade(&db, strategy_a, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        let trade_b = seed_trade(&db, strategy_b, "6758", "sell", 50, 2000, ymd(2026, 6, 2)).await;
        let first_note_id = crate::testing::insert_test_note(&db, "first rationale", "body").await;
        let first_note_version_id =
            super::super::tests_common::current_note_version_id(&db, first_note_id).await;
        let second_note_id =
            crate::testing::insert_test_note(&db, "second rationale", "body").await;
        let second_note_version_id =
            super::super::tests_common::current_note_version_id(&db, second_note_id).await;
        let (
            note_at_first_link_time,
            version_at_first_link_time,
            note_at_second_link_time,
            version_at_second_link_time,
        ) = if first_note_id < second_note_id {
            (
                second_note_id,
                second_note_version_id,
                first_note_id,
                first_note_version_id,
            )
        } else {
            (
                first_note_id,
                first_note_version_id,
                second_note_id,
                second_note_version_id,
            )
        };
        let first_link_time = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset();
        seed_trade_note(
            &db,
            trade_a,
            note_at_first_link_time,
            version_at_first_link_time,
            first_link_time,
        )
        .await;
        seed_trade_note(
            &db,
            trade_a,
            note_at_second_link_time,
            version_at_second_link_time,
            first_link_time + chrono::Duration::seconds(1),
        )
        .await;
        let server = build_server(db);

        let result = server
            .read_trades(strategy_a, ReadTradesParams::default())
            .await
            .expect("read_trades");

        assert_eq!(
            result,
            ReadTradesResult {
                trades: vec![
                    TradeDto {
                        trade_id: trade_b,
                        strategy_id: strategy_b,
                        date: ymd(2026, 6, 2),
                        symbol: "6758".into(),
                        side: "sell".into(),
                        qty: 50.0,
                        price: 2000.0,
                        notes: vec![],
                    },
                    TradeDto {
                        trade_id: trade_a,
                        strategy_id: strategy_a,
                        date: ymd(2026, 6, 1),
                        symbol: "7203".into(),
                        side: "buy".into(),
                        qty: 100.0,
                        price: 1000.0,
                        notes: vec![
                            TradeNoteReferenceDto {
                                note_id: note_at_first_link_time,
                                note_version_id: version_at_first_link_time,
                            },
                            TradeNoteReferenceDto {
                                note_id: note_at_second_link_time,
                                note_version_id: version_at_second_link_time,
                            },
                        ],
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_trades_filters_by_symbol(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        let trade_id =
            seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        seed_trade(&db, strategy_id, "6758", "buy", 50, 2000, ymd(2026, 6, 1)).await;
        let server = build_server(db);

        let result = server
            .read_trades(
                strategy_id,
                ReadTradesParams {
                    symbol: Some("7203".into()),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        assert_eq!(
            result,
            ReadTradesResult {
                trades: vec![TradeDto {
                    trade_id,
                    strategy_id,
                    date: ymd(2026, 6, 1),
                    symbol: "7203".into(),
                    side: "buy".into(),
                    qty: 100.0,
                    price: 1000.0,
                    notes: vec![],
                }],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_trades_filters_by_date_from_inclusive(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        let trade_at_boundary =
            seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 5)).await;
        let trade_after_boundary =
            seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 10)).await;
        let server = build_server(db);

        let result = server
            .read_trades(
                strategy_id,
                ReadTradesParams {
                    date_from: Some(ymd(2026, 6, 5)),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        assert_eq!(
            result,
            ReadTradesResult {
                trades: vec![
                    TradeDto {
                        trade_id: trade_after_boundary,
                        strategy_id,
                        date: ymd(2026, 6, 10),
                        symbol: "7203".into(),
                        side: "buy".into(),
                        qty: 100.0,
                        price: 1000.0,
                        notes: vec![],
                    },
                    TradeDto {
                        trade_id: trade_at_boundary,
                        strategy_id,
                        date: ymd(2026, 6, 5),
                        symbol: "7203".into(),
                        side: "buy".into(),
                        qty: 100.0,
                        price: 1000.0,
                        notes: vec![],
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn read_trades_respects_limit(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "a").await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 1)).await;
        seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 2)).await;
        let newest_trade =
            seed_trade(&db, strategy_id, "7203", "buy", 100, 1000, ymd(2026, 6, 3)).await;
        let server = build_server(db);

        let result = server
            .read_trades(
                strategy_id,
                ReadTradesParams {
                    limit: Some(1),
                    ..Default::default()
                },
            )
            .await
            .expect("read_trades");

        assert_eq!(
            result,
            ReadTradesResult {
                trades: vec![TradeDto {
                    trade_id: newest_trade,
                    strategy_id,
                    date: ymd(2026, 6, 3),
                    symbol: "7203".into(),
                    side: "buy".into(),
                    qty: 100.0,
                    price: 1000.0,
                    notes: vec![],
                }],
            },
        );
    }
}
