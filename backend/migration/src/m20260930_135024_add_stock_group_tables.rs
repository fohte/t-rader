use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260930_135024_add_stock_group_tables"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GroupAxis::Table)
                    .col(
                        ColumnDef::new(GroupAxis::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(GroupAxis::Key)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(GroupAxis::Name).text().not_null())
                    .col(ColumnDef::new(GroupAxis::Description).text().not_null())
                    .col(ColumnDef::new(GroupAxis::SyncSource).text())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StockGroup::Table)
                    .col(
                        ColumnDef::new(StockGroup::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(StockGroup::AxisId).uuid().not_null())
                    .col(ColumnDef::new(StockGroup::Key).text().not_null())
                    .col(ColumnDef::new(StockGroup::Name).text().not_null())
                    .col(ColumnDef::new(StockGroup::Description).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(StockGroup::Table, StockGroup::AxisId)
                            .to(GroupAxis::Table, GroupAxis::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::StockGroupAxisIdKey.to_string())
                    .table(StockGroup::Table)
                    .col(StockGroup::AxisId)
                    .col(StockGroup::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StockGroupMember::Table)
                    .col(ColumnDef::new(StockGroupMember::StockId).string().not_null())
                    .col(ColumnDef::new(StockGroupMember::GroupId).uuid().not_null())
                    .col(
                        ColumnDef::new(StockGroupMember::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(StockGroupMember::StockId)
                            .col(StockGroupMember::GroupId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(StockGroupMember::Table, StockGroupMember::StockId)
                            .to(Stock::Table, Stock::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(StockGroupMember::Table, StockGroupMember::GroupId)
                            .to(StockGroup::Table, StockGroup::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::StockGroupMemberGroupId.to_string())
                    .table(StockGroupMember::Table)
                    .col(StockGroupMember::GroupId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(StockGroupMember::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StockGroup::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GroupAxis::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GroupAxis {
    Table,
    Id,
    Key,
    Name,
    Description,
    SyncSource,
}

#[derive(DeriveIden)]
enum StockGroup {
    Table,
    Id,
    AxisId,
    Key,
    Name,
    Description,
}

#[derive(DeriveIden)]
enum StockGroupMember {
    Table,
    StockId,
    GroupId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Stock {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_stock_group_axis_id_key")]
    StockGroupAxisIdKey,
    #[sea_orm(iden = "idx_stock_group_member_group_id")]
    StockGroupMemberGroupId,
}
