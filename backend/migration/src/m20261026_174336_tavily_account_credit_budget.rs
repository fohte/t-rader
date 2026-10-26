use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum TavilyCreditBudgetState {
    Table,
    SingletonKey,
    AccountId,
    DailyLimit,
    MonthlyLimit,
    DayStart,
    DailyUsed,
    DailyCarry,
    DayProviderUsage,
    DailyBlockedUntil,
    MonthStart,
    MonthlyUsed,
    MonthlyCarry,
    ProviderUsage,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TavilyCreditReservation {
    Table,
    ReservationId,
    CallId,
    RequestFingerprint,
    Operation,
    State,
    ReservedCredits,
    SettledCredits,
    DayStart,
    MonthStart,
    OutcomeJson,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TavilyCreditBudgetState::Table)
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::SingletonKey)
                            .small_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::AccountId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DailyLimit)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::MonthlyLimit)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DayStart)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DailyUsed)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DailyCarry)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DayProviderUsage)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::DailyBlockedUntil)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::MonthStart)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::MonthlyUsed)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::MonthlyCarry)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::ProviderUsage)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditBudgetState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(TavilyCreditBudgetState::SingletonKey).eq(1))
                    .check(Expr::col(TavilyCreditBudgetState::DailyLimit).gt(0))
                    .check(Expr::col(TavilyCreditBudgetState::MonthlyLimit).gt(0))
                    .check(Expr::col(TavilyCreditBudgetState::DailyUsed).gte(0))
                    .check(Expr::col(TavilyCreditBudgetState::DailyCarry).gte(0))
                    .check(
                        Expr::col(TavilyCreditBudgetState::DailyCarry)
                            .lte(Expr::col(TavilyCreditBudgetState::DailyUsed)),
                    )
                    .check(Expr::col(TavilyCreditBudgetState::DayProviderUsage).gte(0))
                    .check(Expr::col(TavilyCreditBudgetState::MonthlyUsed).gte(0))
                    .check(Expr::col(TavilyCreditBudgetState::MonthlyCarry).gte(0))
                    .check(
                        Expr::col(TavilyCreditBudgetState::MonthlyCarry)
                            .lte(Expr::col(TavilyCreditBudgetState::MonthlyUsed)),
                    )
                    .check(Expr::col(TavilyCreditBudgetState::ProviderUsage).gte(0))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TavilyCreditReservation::Table)
                    .col(
                        ColumnDef::new(TavilyCreditReservation::ReservationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::CallId)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::RequestFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::Operation)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::State)
                            .string_len(24)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::ReservedCredits)
                            .decimal_len(12, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::SettledCredits)
                            .decimal_len(12, 6),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::DayStart)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::MonthStart)
                            .date()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TavilyCreditReservation::OutcomeJson).json_binary())
                    .col(
                        ColumnDef::new(TavilyCreditReservation::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TavilyCreditReservation::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(TavilyCreditReservation::Operation).is_in(["search", "extract"]))
                    .check(Expr::col(TavilyCreditReservation::State).is_in([
                        "prepared",
                        "dispatching",
                        "settled",
                        "unknown",
                        "provider_rejected",
                        "denied",
                    ]))
                    .check(Expr::col(TavilyCreditReservation::ReservedCredits).gte(0))
                    .check(
                        Expr::col(TavilyCreditReservation::SettledCredits)
                            .is_null()
                            .or(Expr::col(TavilyCreditReservation::SettledCredits).gte(0)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tavily_credit_reservation_period_state")
                    .table(TavilyCreditReservation::Table)
                    .col(TavilyCreditReservation::MonthStart)
                    .col(TavilyCreditReservation::DayStart)
                    .col(TavilyCreditReservation::State)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TavilyCreditReservation::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TavilyCreditBudgetState::Table).to_owned())
            .await?;
        Ok(())
    }
}
