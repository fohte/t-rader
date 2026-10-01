use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260930_154107_replace_reference_kinds_with_groups"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DELETE FROM note_ref WHERE ref_kind IN ('sector', 'theme')",
        )
        .await?;
        db.execute_unprepared("ALTER TABLE note_ref DROP CONSTRAINT note_ref_ref_kind_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE note_ref ADD CONSTRAINT note_ref_ref_kind_check \
             CHECK (ref_kind IN ('stock', 'indicator', 'group'))",
        )
        .await?;
        db.execute_unprepared("DELETE FROM ref_term WHERE ref_kind IN ('sector', 'theme')")
            .await?;
        db.execute_unprepared("ALTER TABLE ref_term DROP CONSTRAINT ref_term_ref_kind_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE ref_term ADD CONSTRAINT ref_term_ref_kind_check \
             CHECK (ref_kind IN ('stock', 'indicator', 'group'))",
        )
        .await?;

        manager
            .drop_table(Table::drop().table(Theme::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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

        let db = manager.get_connection();
        db.execute_unprepared("DELETE FROM note_ref WHERE ref_kind = 'group'")
            .await?;
        db.execute_unprepared("ALTER TABLE note_ref DROP CONSTRAINT note_ref_ref_kind_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE note_ref ADD CONSTRAINT note_ref_ref_kind_check \
             CHECK (ref_kind IN ('stock', 'indicator', 'sector', 'theme'))",
        )
        .await?;
        db.execute_unprepared("DELETE FROM ref_term WHERE ref_kind = 'group'")
            .await?;
        db.execute_unprepared("ALTER TABLE ref_term DROP CONSTRAINT ref_term_ref_kind_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE ref_term ADD CONSTRAINT ref_term_ref_kind_check \
             CHECK (ref_kind IN ('stock', 'indicator', 'sector', 'theme'))",
        )
        .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Theme {
    Table,
    Id,
    Name,
    Description,
}
