use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    RiskPolicy,
}

#[derive(DeriveIden)]
enum AccountRiskPolicy {
    Table,
    Id,
    RiskPolicy,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::RiskPolicy)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .to_owned(),
            )
            .await?;

        // 口座全体の設定は単一行のみを持つ (id は常に 1 固定)。行が存在しない間は
        // 「未設定」を表し、初回 PUT で upsert する (`services::account_risk_policy`)。
        manager
            .create_table(
                Table::create()
                    .table(AccountRiskPolicy::Table)
                    .col(
                        ColumnDef::new(AccountRiskPolicy::Id)
                            .small_integer()
                            .not_null()
                            .primary_key()
                            .default(1)
                            .check(Expr::col(AccountRiskPolicy::Id).eq(1)),
                    )
                    .col(
                        ColumnDef::new(AccountRiskPolicy::RiskPolicy)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(AccountRiskPolicy::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AccountRiskPolicy::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .drop_column(Strategy::RiskPolicy)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
