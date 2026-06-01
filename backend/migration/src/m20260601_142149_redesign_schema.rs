use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// 参照型 (一級型) 共通の kind 値
const REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];
// アーティファクト共通の status 値
const STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
// 産出元 (人間か LLM か)
const ORIGINS: [&str; 2] = ["human", "llm"];

#[derive(DeriveIden)]
enum Strategy {
    Table,
    Id,
    Name,
    Description,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Sector {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
    Name,
    Market,
    SectorId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Indicator {
    Table,
    Id,
    Name,
    Kind,
}

#[derive(DeriveIden)]
enum Theme {
    Table,
    Id,
    Name,
    Description,
}

#[derive(DeriveIden)]
enum StrategyInterest {
    Table,
    StrategyId,
    RefKind,
    RefId,
    Role,
    Origin,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
    StrategyId,
    Title,
    BodyMd,
    FrontmatterJson,
    TypeTag,
    Status,
    Trigger,
    TriggerLabel,
    CreatedByKind,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum NoteRef {
    Table,
    NoteId,
    RefKind,
    RefId,
}

#[derive(DeriveIden)]
enum Annotation {
    Table,
    Id,
    StrategyId,
    TargetSymbol,
    TargetKind,
    Timestamp,
    Price,
    Text,
    Status,
    LinkedNoteId,
    CreatedByKind,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Comment {
    Table,
    Id,
    TargetKind,
    TargetId,
    ParentId,
    Body,
    AuthorKind,
    AuthorLabel,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ChangeHistory {
    Table,
    Id,
    TargetKind,
    TargetId,
    ActorKind,
    ActorLabel,
    Op,
    DiffJson,
    Summary,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Trade {
    Table,
    Id,
    StrategyId,
    Symbol,
    Side,
    Qty,
    Price,
    Fee,
    Date,
    Source,
    Note,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PortfolioSnapshot {
    Table,
    Id,
    TakenAt,
    CashJpy,
    TotalEquityJpy,
    PositionsJson,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Strategy::Table)
                    .col(ColumnDef::new(Strategy::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Strategy::Name).string().not_null())
                    .col(ColumnDef::new(Strategy::Description).text())
                    .col(
                        ColumnDef::new(Strategy::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Strategy::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Strategy::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Sector::Table)
                    .col(ColumnDef::new(Sector::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Sector::Name).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Stock::Table)
                    .col(ColumnDef::new(Stock::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Stock::Name).string().not_null())
                    .col(ColumnDef::new(Stock::Market).string())
                    .col(ColumnDef::new(Stock::SectorId).string())
                    .col(
                        ColumnDef::new(Stock::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Stock::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Stock::Table, Stock::SectorId)
                            .to(Sector::Table, Sector::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_stock_sector_id")
                    .table(Stock::Table)
                    .col(Stock::SectorId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Indicator::Table)
                    .col(
                        ColumnDef::new(Indicator::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Indicator::Name).string().not_null())
                    .col(ColumnDef::new(Indicator::Kind).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Theme::Table)
                    .col(ColumnDef::new(Theme::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Theme::Name).string().not_null())
                    .col(ColumnDef::new(Theme::Description).text())
                    .to_owned(),
            )
            .await?;

        // (ref_kind, ref_id) で 4 種の参照型を polymorphic に保持。物理的な FK は張れない
        manager
            .create_table(
                Table::create()
                    .table(StrategyInterest::Table)
                    .col(
                        ColumnDef::new(StrategyInterest::StrategyId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StrategyInterest::RefKind)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(StrategyInterest::RefId).string().not_null())
                    .col(ColumnDef::new(StrategyInterest::Role).string().not_null())
                    .col(ColumnDef::new(StrategyInterest::Origin).string().not_null())
                    .col(
                        ColumnDef::new(StrategyInterest::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(StrategyInterest::StrategyId)
                            .col(StrategyInterest::RefKind)
                            .col(StrategyInterest::RefId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(StrategyInterest::Table, StrategyInterest::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(StrategyInterest::RefKind).is_in(REF_KINDS))
                    .check(Expr::col(StrategyInterest::Role).is_in(["seed", "derived"]))
                    .check(Expr::col(StrategyInterest::Origin).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_strategy_interest_ref_kind_id")
                    .table(StrategyInterest::Table)
                    .col(StrategyInterest::RefKind)
                    .col(StrategyInterest::RefId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Note::Table)
                    .col(ColumnDef::new(Note::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Note::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Note::Title).string().not_null())
                    .col(ColumnDef::new(Note::BodyMd).text().not_null())
                    .col(
                        ColumnDef::new(Note::FrontmatterJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(ColumnDef::new(Note::TypeTag).string())
                    .col(
                        ColumnDef::new(Note::Status)
                            .string()
                            .not_null()
                            .default("unread"),
                    )
                    .col(ColumnDef::new(Note::Trigger).string())
                    .col(ColumnDef::new(Note::TriggerLabel).string())
                    .col(ColumnDef::new(Note::CreatedByKind).string().not_null())
                    .col(
                        ColumnDef::new(Note::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Note::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Note::Table, Note::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Note::Status).is_in(STATUSES))
                    .check(
                        Expr::col(Note::Trigger)
                            .is_null()
                            .or(Expr::col(Note::Trigger).is_in([
                                "hook",
                                "cron",
                                "on-demand",
                                "manual",
                            ])),
                    )
                    .check(Expr::col(Note::CreatedByKind).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_note_strategy_id")
                    .table(Note::Table)
                    .col(Note::StrategyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NoteRef::Table)
                    .col(ColumnDef::new(NoteRef::NoteId).uuid().not_null())
                    .col(ColumnDef::new(NoteRef::RefKind).string().not_null())
                    .col(ColumnDef::new(NoteRef::RefId).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(NoteRef::NoteId)
                            .col(NoteRef::RefKind)
                            .col(NoteRef::RefId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteRef::Table, NoteRef::NoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(NoteRef::RefKind).is_in(REF_KINDS))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_note_ref_kind_id")
                    .table(NoteRef::Table)
                    .col(NoteRef::RefKind)
                    .col(NoteRef::RefId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Annotation::Table)
                    .col(
                        ColumnDef::new(Annotation::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Annotation::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Annotation::TargetSymbol).string().not_null())
                    .col(ColumnDef::new(Annotation::TargetKind).string().not_null())
                    .col(
                        ColumnDef::new(Annotation::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Annotation::Price).decimal())
                    .col(ColumnDef::new(Annotation::Text).text().not_null())
                    .col(
                        ColumnDef::new(Annotation::Status)
                            .string()
                            .not_null()
                            .default("unread"),
                    )
                    .col(ColumnDef::new(Annotation::LinkedNoteId).uuid())
                    .col(
                        ColumnDef::new(Annotation::CreatedByKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Annotation::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Annotation::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Annotation::Table, Annotation::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Annotation::Table, Annotation::LinkedNoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .check(Expr::col(Annotation::TargetKind).is_in([
                        "signal",
                        "level",
                        "observation",
                        "other",
                    ]))
                    .check(Expr::col(Annotation::Status).is_in(STATUSES))
                    .check(Expr::col(Annotation::CreatedByKind).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_annotation_strategy_id")
                    .table(Annotation::Table)
                    .col(Annotation::StrategyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_annotation_target_symbol_timestamp")
                    .table(Annotation::Table)
                    .col(Annotation::TargetSymbol)
                    .col(Annotation::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Comment::Table)
                    .col(ColumnDef::new(Comment::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Comment::TargetKind).string().not_null())
                    .col(ColumnDef::new(Comment::TargetId).uuid().not_null())
                    .col(ColumnDef::new(Comment::ParentId).uuid())
                    .col(ColumnDef::new(Comment::Body).text().not_null())
                    .col(ColumnDef::new(Comment::AuthorKind).string().not_null())
                    .col(ColumnDef::new(Comment::AuthorLabel).string().not_null())
                    .col(
                        ColumnDef::new(Comment::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Comment::Table, Comment::ParentId)
                            .to(Comment::Table, Comment::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Comment::TargetKind).is_in(["note", "annotation"]))
                    .check(Expr::col(Comment::AuthorKind).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_comment_target")
                    .table(Comment::Table)
                    .col(Comment::TargetKind)
                    .col(Comment::TargetId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_comment_parent_id")
                    .table(Comment::Table)
                    .col(Comment::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ChangeHistory::Table)
                    .col(
                        ColumnDef::new(ChangeHistory::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChangeHistory::TargetKind)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ChangeHistory::TargetId).uuid().not_null())
                    .col(ColumnDef::new(ChangeHistory::ActorKind).string().not_null())
                    .col(
                        ColumnDef::new(ChangeHistory::ActorLabel)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ChangeHistory::Op).string().not_null())
                    .col(
                        ColumnDef::new(ChangeHistory::DiffJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ChangeHistory::Summary).text())
                    .col(
                        ColumnDef::new(ChangeHistory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::col(ChangeHistory::TargetKind).is_in([
                        "note",
                        "annotation",
                        "strategy",
                        "trade",
                        "comment",
                    ]))
                    .check(Expr::col(ChangeHistory::ActorKind).is_in(ORIGINS))
                    .check(Expr::col(ChangeHistory::Op).is_in([
                        "create",
                        "update",
                        "delete",
                        "status_change",
                    ]))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_change_history_target")
                    .table(ChangeHistory::Table)
                    .col(ChangeHistory::TargetKind)
                    .col(ChangeHistory::TargetId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Trade::Table)
                    .col(ColumnDef::new(Trade::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Trade::StrategyId).uuid().not_null())
                    .col(ColumnDef::new(Trade::Symbol).string().not_null())
                    .col(ColumnDef::new(Trade::Side).string().not_null())
                    .col(ColumnDef::new(Trade::Qty).decimal().not_null())
                    .col(ColumnDef::new(Trade::Price).decimal().not_null())
                    .col(ColumnDef::new(Trade::Fee).decimal().not_null().default(0))
                    .col(ColumnDef::new(Trade::Date).date().not_null())
                    .col(ColumnDef::new(Trade::Source).string().not_null())
                    .col(ColumnDef::new(Trade::Note).text())
                    .col(
                        ColumnDef::new(Trade::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Trade::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Trade::Table, Trade::StrategyId)
                            .to(Strategy::Table, Strategy::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(Trade::Side).is_in(["buy", "sell"]))
                    .check(Expr::col(Trade::Source).is_in(["manual", "csv", "api"]))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_trade_strategy_id")
                    .table(Trade::Table)
                    .col(Trade::StrategyId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_trade_symbol_date")
                    .table(Trade::Table)
                    .col(Trade::Symbol)
                    .col(Trade::Date)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PortfolioSnapshot::Table)
                    .col(
                        ColumnDef::new(PortfolioSnapshot::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::TakenAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::CashJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::TotalEquityJpy)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::PositionsJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortfolioSnapshot::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_portfolio_snapshot_taken_at")
                    .table(PortfolioSnapshot::Table)
                    .col(PortfolioSnapshot::TakenAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 外部キー依存の逆順で削除
        for table in [
            PortfolioSnapshot::Table.into_iden(),
            Trade::Table.into_iden(),
            ChangeHistory::Table.into_iden(),
            Comment::Table.into_iden(),
            Annotation::Table.into_iden(),
            NoteRef::Table.into_iden(),
            Note::Table.into_iden(),
            StrategyInterest::Table.into_iden(),
            Theme::Table.into_iden(),
            Indicator::Table.into_iden(),
            Stock::Table.into_iden(),
            Sector::Table.into_iden(),
            Strategy::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(table).to_owned())
                .await?;
        }
        Ok(())
    }
}
