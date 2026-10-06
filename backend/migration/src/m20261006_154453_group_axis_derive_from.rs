use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(GroupAxis::Table)
                    .rename_column(GroupAxis::SyncSource, GroupAxis::DeriveFrom)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(StockGroup::Table)
                    .rename_column(StockGroup::SyncSourceCode, StockGroup::Code)
                    .to_owned(),
            )
            .await?;
        manager
            .exec_stmt(
                Query::update()
                    .table(GroupAxis::Table)
                    .value(GroupAxis::DeriveFrom, Expr::value("tse_sector33"))
                    .and_where(Expr::col(GroupAxis::DeriveFrom).eq("jquants"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .exec_stmt(
                Query::update()
                    .table(GroupAxis::Table)
                    .value(GroupAxis::DeriveFrom, Expr::value("jquants"))
                    .and_where(Expr::col(GroupAxis::DeriveFrom).eq("tse_sector33"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(StockGroup::Table)
                    .rename_column(StockGroup::Code, StockGroup::SyncSourceCode)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(GroupAxis::Table)
                    .rename_column(GroupAxis::DeriveFrom, GroupAxis::SyncSource)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum GroupAxis {
    Table,
    SyncSource,
    DeriveFrom,
}

#[derive(DeriveIden)]
enum StockGroup {
    Table,
    SyncSourceCode,
    Code,
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "テスト準備の失敗時に詳細を表示するため"
    )]

    use sea_orm::sea_query::{Alias, Query};
    use sea_orm::{ConnectionTrait, Database, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn migration_renames_attributes_and_preserves_groups_and_memberships() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("group_axis_derive_from_test_{}", std::process::id());
        transaction
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        transaction
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}"))
            .await
            .expect("set isolated schema");
        transaction
            .execute_unprepared(
                "CREATE TABLE group_axis (
                    id text PRIMARY KEY,
                    key text NOT NULL,
                    name text NOT NULL,
                    description text NOT NULL,
                    sync_source text
                )",
            )
            .await
            .expect("create group axis table");
        transaction
            .execute_unprepared(
                "CREATE TABLE stock_group (
                    id text PRIMARY KEY,
                    axis_id text NOT NULL,
                    key text NOT NULL,
                    name text NOT NULL,
                    description text,
                    sync_source_code text
                )",
            )
            .await
            .expect("create stock group table");
        transaction
            .execute_unprepared(
                "CREATE TABLE stock_group_member (
                    stock_id text NOT NULL,
                    group_id text NOT NULL
                )",
            )
            .await
            .expect("create stock group member table");
        transaction
            .execute_unprepared(
                "INSERT INTO group_axis (id, key, name, description, sync_source)
                 VALUES ('00000000-0000-0000-0000-000000000001', 'sample-axis', 'Sample axis', 'A synthetic classification axis', 'jquants')",
            )
            .await
            .expect("insert group axis");
        transaction
            .execute_unprepared(
                "INSERT INTO stock_group (id, axis_id, key, name, description, sync_source_code)
                 VALUES ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', 'sample-group', 'Sample group', 'A synthetic group description', '1234')",
            )
            .await
            .expect("insert stock group");
        transaction
            .execute_unprepared(
                "INSERT INTO stock_group_member (stock_id, group_id)
                 VALUES ('DEMO-STOCK-A', '00000000-0000-0000-0000-000000000002')",
            )
            .await
            .expect("insert stock group member");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run attribute rename migration");

        let migrated = (
            select_strings(
                &transaction,
                "group_axis",
                &["id", "key", "name", "description", "derive_from"],
            )
            .await,
            select_strings(
                &transaction,
                "stock_group",
                &["id", "axis_id", "key", "name", "description", "code"],
            )
            .await,
            select_strings(&transaction, "stock_group_member", &["stock_id", "group_id"]).await,
        );
        assert_eq!(
            migrated,
            (
                vec![vec![
                    "00000000-0000-0000-0000-000000000001".into(),
                    "sample-axis".into(),
                    "Sample axis".into(),
                    "A synthetic classification axis".into(),
                    "tse_sector33".into(),
                ]],
                vec![vec![
                    "00000000-0000-0000-0000-000000000002".into(),
                    "00000000-0000-0000-0000-000000000001".into(),
                    "sample-group".into(),
                    "Sample group".into(),
                    "A synthetic group description".into(),
                    "1234".into(),
                ]],
                vec![vec![
                    "DEMO-STOCK-A".into(),
                    "00000000-0000-0000-0000-000000000002".into(),
                ]],
            ),
        );

        Migration
            .down(&SchemaManager::new(&transaction))
            .await
            .expect("run attribute rename rollback");

        let rolled_back = (
            select_strings(
                &transaction,
                "group_axis",
                &["id", "key", "name", "description", "sync_source"],
            )
            .await,
            select_strings(
                &transaction,
                "stock_group",
                &[
                    "id",
                    "axis_id",
                    "key",
                    "name",
                    "description",
                    "sync_source_code",
                ],
            )
            .await,
            select_strings(&transaction, "stock_group_member", &["stock_id", "group_id"]).await,
        );
        assert_eq!(
            rolled_back,
            (
                vec![vec![
                    "00000000-0000-0000-0000-000000000001".into(),
                    "sample-axis".into(),
                    "Sample axis".into(),
                    "A synthetic classification axis".into(),
                    "jquants".into(),
                ]],
                vec![vec![
                    "00000000-0000-0000-0000-000000000002".into(),
                    "00000000-0000-0000-0000-000000000001".into(),
                    "sample-group".into(),
                    "Sample group".into(),
                    "A synthetic group description".into(),
                    "1234".into(),
                ]],
                vec![vec![
                    "DEMO-STOCK-A".into(),
                    "00000000-0000-0000-0000-000000000002".into(),
                ]],
            ),
        );
        transaction.rollback().await.expect("roll back test schema");
    }

    async fn select_strings(
        transaction: &impl ConnectionTrait,
        table: &str,
        columns: &[&str],
    ) -> Vec<Vec<String>> {
        transaction
            .query_all(
                &Query::select()
                    .columns(columns.iter().map(|column| Alias::new(*column)))
                    .from(Alias::new(table))
                    .to_owned(),
            )
            .await
            .expect("query migrated rows")
            .into_iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|column| {
                        row.try_get::<String>("", column)
                            .expect("read migrated column")
                    })
                    .collect()
            })
            .collect()
    }
}
