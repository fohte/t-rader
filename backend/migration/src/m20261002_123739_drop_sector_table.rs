use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261002_123739_drop_sector_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Stock::Table)
                    .drop_column(Stock::SectorId)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Sector::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
            .alter_table(
                Table::alter()
                    .table(Stock::Table)
                    .add_column(ColumnDef::new(Stock::SectorId).string())
                    .to_owned(),
            )
            .await?;

        let sector_foreign_key = TableForeignKey::new()
            .name("fk_stock_sector_id_sector")
            .from_tbl(Stock::Table)
            .from_col(Stock::SectorId)
            .to_tbl(Sector::Table)
            .to_col(Sector::Id)
            .on_delete(ForeignKeyAction::SetNull)
            .to_owned();
        manager
            .alter_table(
                Table::alter()
                    .table(Stock::Table)
                    .add_foreign_key(&sector_foreign_key)
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

        let db = manager.get_connection();
        db.execute_unprepared(
            "INSERT INTO sector (id, name) \
             SELECT DISTINCT sg.key, sg.key \
             FROM stock_group sg \
             JOIN group_axis ga ON ga.id = sg.axis_id \
             WHERE ga.sync_source = 'jquants' \
             ON CONFLICT (id) DO NOTHING",
        )
        .await?;
        db.execute_unprepared(
            "UPDATE stock s \
             SET sector_id = ranked.sector_id \
             FROM ( \
                 SELECT DISTINCT ON (sgm.stock_id) sgm.stock_id, sg.key AS sector_id \
                 FROM stock_group_member sgm \
                 JOIN stock_group sg ON sg.id = sgm.group_id \
                 JOIN group_axis ga ON ga.id = sg.axis_id \
                 WHERE ga.sync_source = 'jquants' \
                 ORDER BY sgm.stock_id, ga.key, sg.key \
             ) ranked \
             WHERE s.id = ranked.stock_id",
        )
        .await?;

        Ok(())
    }
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
    SectorId,
}
