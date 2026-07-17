use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Strategy {
    Table,
    AgentStatus,
    AgentError,
}

#[derive(DeriveIden)]
struct StrategyAgentStatus;

#[derive(DeriveIden)]
enum StrategyAgentStatusVariant {
    Pending,
    Ready,
    Failed,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .drop_column(Strategy::AgentError)
                    .drop_column(Strategy::AgentStatus)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(Type::drop().name(StrategyAgentStatus).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(StrategyAgentStatus)
                    .values([
                        StrategyAgentStatusVariant::Pending,
                        StrategyAgentStatusVariant::Ready,
                        StrategyAgentStatusVariant::Failed,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Strategy::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Strategy::AgentStatus)
                            .custom(StrategyAgentStatus)
                            .not_null()
                            .default(Expr::cust("'pending'::strategy_agent_status")),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Strategy::AgentError).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
