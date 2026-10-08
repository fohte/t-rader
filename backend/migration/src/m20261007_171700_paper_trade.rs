use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum PaperAccount {
    Table,
    Id,
    Name,
    StrategyId,
    Purpose,
    InitialCashJpy,
    BenchmarkStockId,
    StartedOn,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PaperOrder {
    Table,
    Id,
    AccountId,
    StockId,
    Side,
    Qty,
    NoteVersionId,
    OrderedAt,
}

#[derive(DeriveIden)]
enum PaperOrderResult {
    Table,
    OrderId,
    Outcome,
    FillDate,
    FillPrice,
    RejectReason,
    DecidedAt,
}

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum AgentConfig {
    Table,
    Purpose,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NoteVersion {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PaperAccount::Table)
                    .col(
                        ColumnDef::new(PaperAccount::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(PaperAccount::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(PaperAccount::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(PaperAccount::Purpose).text().not_null())
                    .col(
                        ColumnDef::new(PaperAccount::InitialCashJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PaperAccount::BenchmarkStockId).string())
                    .col(ColumnDef::new(PaperAccount::StartedOn).date().not_null())
                    .col(
                        ColumnDef::new(PaperAccount::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaperAccount::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperAccount::Table, PaperAccount::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperAccount::Table, PaperAccount::Purpose)
                            .to(AgentConfig::Table, AgentConfig::Purpose)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperAccount::Table, PaperAccount::BenchmarkStockId)
                            .to(Stock::Table, Stock::Id),
                    )
                    .check(Expr::col(PaperAccount::InitialCashJpy).gt(0))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_paper_account_strategy_purpose")
                    .table(PaperAccount::Table)
                    .col(PaperAccount::StrategyId)
                    .col(PaperAccount::Purpose)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_paper_account_purpose")
                    .table(PaperAccount::Table)
                    .col(PaperAccount::Purpose)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_paper_account_benchmark_stock_id")
                    .table(PaperAccount::Table)
                    .col(PaperAccount::BenchmarkStockId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PaperOrder::Table)
                    .col(
                        ColumnDef::new(PaperOrder::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(PaperOrder::AccountId).uuid().not_null())
                    .col(ColumnDef::new(PaperOrder::StockId).string().not_null())
                    .col(ColumnDef::new(PaperOrder::Side).text().not_null())
                    .col(ColumnDef::new(PaperOrder::Qty).big_integer().not_null())
                    .col(ColumnDef::new(PaperOrder::NoteVersionId).uuid().not_null())
                    .col(
                        ColumnDef::new(PaperOrder::OrderedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperOrder::Table, PaperOrder::AccountId)
                            .to(PaperAccount::Table, PaperAccount::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperOrder::Table, PaperOrder::StockId)
                            .to(Stock::Table, Stock::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperOrder::Table, PaperOrder::NoteVersionId)
                            .to(NoteVersion::Table, NoteVersion::Id),
                    )
                    .check(Expr::col(PaperOrder::Side).is_in(["buy", "sell"]))
                    .check(
                        Expr::col(PaperOrder::Qty)
                            .gt(0)
                            .and(Expr::col(PaperOrder::Qty).modulo(100).eq(0)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_paper_order_account_ordered_at")
                    .table(PaperOrder::Table)
                    .col(PaperOrder::AccountId)
                    .col(PaperOrder::OrderedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_paper_order_stock_id")
                    .table(PaperOrder::Table)
                    .col(PaperOrder::StockId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_paper_order_note_version_id")
                    .table(PaperOrder::Table)
                    .col(PaperOrder::NoteVersionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PaperOrderResult::Table)
                    .col(
                        ColumnDef::new(PaperOrderResult::OrderId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PaperOrderResult::Outcome).text().not_null())
                    .col(ColumnDef::new(PaperOrderResult::FillDate).date())
                    .col(ColumnDef::new(PaperOrderResult::FillPrice).decimal())
                    .col(ColumnDef::new(PaperOrderResult::RejectReason).text())
                    .col(
                        ColumnDef::new(PaperOrderResult::DecidedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaperOrderResult::Table, PaperOrderResult::OrderId)
                            .to(PaperOrder::Table, PaperOrder::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(PaperOrderResult::Outcome).is_in(["filled", "rejected"]))
                    .check(
                        Expr::col(PaperOrderResult::Outcome)
                            .eq("filled")
                            .and(
                                Expr::col(PaperOrderResult::FillDate)
                                    .is_not_null()
                                    .and(Expr::col(PaperOrderResult::FillPrice).is_not_null())
                                    .and(Expr::col(PaperOrderResult::RejectReason).is_null()),
                            )
                            .or(Expr::col(PaperOrderResult::Outcome).eq("rejected").and(
                                Expr::col(PaperOrderResult::FillDate)
                                    .is_null()
                                    .and(Expr::col(PaperOrderResult::FillPrice).is_null())
                                    .and(Expr::col(PaperOrderResult::RejectReason).is_not_null()),
                            )),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PaperOrderResult::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PaperOrder::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PaperAccount::Table).to_owned())
            .await
    }
}
