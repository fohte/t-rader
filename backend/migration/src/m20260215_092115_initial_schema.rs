use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // TimescaleDB 拡張の有効化
        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await?;

        // instruments テーブル
        db.execute_unprepared(
            "CREATE TABLE instruments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                market TEXT NOT NULL CHECK (market IN ('TSE')),
                sector TEXT
            )",
        )
        .await?;

        // watchlists テーブル
        db.execute_unprepared(
            "CREATE TABLE watchlists (
                id UUID PRIMARY KEY,
                name TEXT NOT NULL,
                sort_order INTEGER NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .await?;

        // watchlist_items テーブル
        db.execute_unprepared(
            "CREATE TABLE watchlist_items (
                watchlist_id UUID NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
                instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                sort_order INTEGER NOT NULL,
                added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                PRIMARY KEY (watchlist_id, instrument_id)
            )",
        )
        .await?;

        // bars テーブル
        db.execute_unprepared(
            "CREATE TABLE bars (
                instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                timeframe TEXT NOT NULL CHECK (timeframe IN ('1d')),
                timestamp TIMESTAMPTZ NOT NULL,
                open NUMERIC NOT NULL,
                high NUMERIC NOT NULL,
                low NUMERIC NOT NULL,
                close NUMERIC NOT NULL,
                volume BIGINT NOT NULL,
                PRIMARY KEY (instrument_id, timeframe, timestamp)
            )",
        )
        .await?;

        // TimescaleDB hypertable 化
        db.execute_unprepared("SELECT create_hypertable('bars', by_range('timestamp'))")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP TABLE IF EXISTS bars CASCADE")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS watchlist_items CASCADE")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS watchlists CASCADE")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS instruments CASCADE")
            .await?;

        Ok(())
    }
}
