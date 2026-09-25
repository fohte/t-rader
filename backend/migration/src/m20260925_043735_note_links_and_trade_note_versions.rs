use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_043735_note_links_and_trade_note_versions"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TradeNote::Table)
                    .add_column(ColumnDef::new(TradeNote::NoteVersionId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE trade_note AS trade_note_row
                SET note_version_id = note_version.id
                FROM note_version
                WHERE note_version.note_id = trade_note_row.note_id
                  AND note_version.is_current;
                "#,
            )
            .await?;

        let mut trade_note_version_fk = TableForeignKey::new();
        trade_note_version_fk
            .name("fk_trade_note_note_version")
            .from_tbl(TradeNote::Table)
            .from_col(TradeNote::NoteVersionId)
            .to_tbl(NoteVersion::Table)
            .to_col(NoteVersion::Id)
            .on_delete(ForeignKeyAction::Cascade);

        manager
            .alter_table(
                Table::alter()
                    .table(TradeNote::Table)
                    .modify_column(ColumnDef::new(TradeNote::NoteVersionId).uuid().not_null())
                    .add_foreign_key(&trade_note_version_fk)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::TradeNoteVersion.to_string())
                    .table(TradeNote::Table)
                    .col(TradeNote::NoteVersionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NoteLink::Table)
                    .col(ColumnDef::new(NoteLink::FromVersionId).uuid().not_null())
                    .col(ColumnDef::new(NoteLink::ToNoteId).uuid().not_null())
                    .col(ColumnDef::new(NoteLink::ToVersionId).uuid())
                    .primary_key(
                        Index::create()
                            .col(NoteLink::FromVersionId)
                            .col(NoteLink::ToNoteId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteLink::Table, NoteLink::FromVersionId)
                            .to(NoteVersion::Table, NoteVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteLink::Table, NoteLink::ToNoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteLink::Table, NoteLink::ToVersionId)
                            .to(NoteVersion::Table, NoteVersion::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::NoteLinkTarget.to_string())
                    .table(NoteLink::Table)
                    .col(NoteLink::ToNoteId)
                    .col(NoteLink::FromVersionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::NoteLinkVersion.to_string())
                    .table(NoteLink::Table)
                    .col(NoteLink::ToVersionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NoteLink::Table).to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(Idx::TradeNoteVersion.to_string())
                    .table(TradeNote::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(TradeNote::Table)
                    .drop_column(TradeNote::NoteVersionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
#[derive(DeriveIden)]
enum TradeNote {
    Table,
    NoteVersionId,
}

#[derive(DeriveIden)]
enum NoteVersion {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NoteLink {
    Table,
    FromVersionId,
    ToNoteId,
    ToVersionId,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "idx_trade_note_note_version_id")]
    TradeNoteVersion,
    #[sea_orm(iden = "idx_note_link_to_note_id_from_version_id")]
    NoteLinkTarget,
    #[sea_orm(iden = "idx_note_link_to_version_id")]
    NoteLinkVersion,
}
