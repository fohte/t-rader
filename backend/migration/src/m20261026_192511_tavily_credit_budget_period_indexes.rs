use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261026_192511_tavily_credit_budget_period_indexes"
    }
}

#[derive(DeriveIden)]
enum TavilyCreditReservation {
    Table,
    DayStart,
    MonthStart,
    State,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_tavily_credit_reservation_day_state")
                    .table(TavilyCreditReservation::Table)
                    .col(TavilyCreditReservation::DayStart)
                    .col(TavilyCreditReservation::State)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_tavily_credit_reservation_month_state")
                    .table(TavilyCreditReservation::Table)
                    .col(TavilyCreditReservation::MonthStart)
                    .col(TavilyCreditReservation::State)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tavily_credit_reservation_month_state")
                    .table(TavilyCreditReservation::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tavily_credit_reservation_day_state")
                    .table(TavilyCreditReservation::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
