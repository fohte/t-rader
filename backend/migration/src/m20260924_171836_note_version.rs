use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260924_171836_note_version"
    }
}

#[derive(DeriveIden)]
enum Note {
    Table,
    Id,
    Title,
    BodyMd,
    FrontmatterJson,
    GraphsJson,
    Status,
    CreatedByKind,
}

#[derive(DeriveIden)]
enum NoteVersion {
    Table,
    Id,
    NoteId,
    VersionNo,
    Title,
    BodyMd,
    FrontmatterJson,
    GraphsJson,
    Status,
    IsCurrent,
    ChangeReason,
    CreatedByKind,
    ExecutionId,
    CreatedAt,
    ReviewedAt,
}

#[derive(DeriveIden)]
enum Idx {
    #[sea_orm(iden = "note_version_note_id_version_no_key")]
    NoteVersionNoteIdVersionNo,
    #[sea_orm(iden = "idx_note_version_current")]
    NoteVersionCurrent,
}

const STATUSES: [&str; 3] = ["approved", "unread", "rejected"];
const ORIGINS: [&str; 2] = ["human", "llm"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NoteVersion::Table)
                    .col(
                        ColumnDef::new(NoteVersion::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(NoteVersion::NoteId).uuid().not_null())
                    .col(ColumnDef::new(NoteVersion::VersionNo).integer().not_null())
                    .col(ColumnDef::new(NoteVersion::Title).text().not_null())
                    .col(ColumnDef::new(NoteVersion::BodyMd).text().not_null())
                    .col(
                        ColumnDef::new(NoteVersion::FrontmatterJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(NoteVersion::GraphsJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(NoteVersion::Status)
                            .text()
                            .not_null()
                            .default("unread"),
                    )
                    .col(
                        ColumnDef::new(NoteVersion::IsCurrent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(NoteVersion::ChangeReason).text())
                    .col(ColumnDef::new(NoteVersion::CreatedByKind).text().not_null())
                    .col(ColumnDef::new(NoteVersion::ExecutionId).text())
                    .col(
                        ColumnDef::new(NoteVersion::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(NoteVersion::ReviewedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(NoteVersion::Table, NoteVersion::NoteId)
                            .to(Note::Table, Note::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(NoteVersion::Status).is_in(STATUSES))
                    .check(Expr::col(NoteVersion::CreatedByKind).is_in(ORIGINS))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(Idx::NoteVersionNoteIdVersionNo.to_string())
                    .table(NoteVersion::Table)
                    .col(NoteVersion::NoteId)
                    .col(NoteVersion::VersionNo)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name(Idx::NoteVersionCurrent.to_string())
                    .table(NoteVersion::Table)
                    .col(NoteVersion::NoteId)
                    .unique()
                    .and_where(Expr::col(NoteVersion::IsCurrent).eq(true))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO note_version (
                    note_id,
                    version_no,
                    title,
                    body_md,
                    frontmatter_json,
                    graphs_json,
                    status,
                    is_current,
                    created_by_kind,
                    execution_id,
                    created_at
                )
                SELECT
                    id,
                    1,
                    title,
                    body_md,
                    frontmatter_json,
                    graphs_json,
                    status,
                    true,
                    created_by_kind,
                    execution_id,
                    updated_at
                FROM note;
                "#,
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .drop_column(Note::Title)
                    .drop_column(Note::BodyMd)
                    .drop_column(Note::FrontmatterJson)
                    .drop_column(Note::GraphsJson)
                    .drop_column(Note::Status)
                    .drop_column(Note::CreatedByKind)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .add_column(ColumnDef::new(Note::Title).text())
                    .add_column(ColumnDef::new(Note::BodyMd).text())
                    .add_column(
                        ColumnDef::new(Note::FrontmatterJson)
                            .json_binary()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .add_column(
                        ColumnDef::new(Note::GraphsJson)
                            .json_binary()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .add_column(ColumnDef::new(Note::Status).text().default("unread"))
                    .add_column(ColumnDef::new(Note::CreatedByKind).text())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE note AS n
                SET
                    title = v.title,
                    body_md = v.body_md,
                    frontmatter_json = v.frontmatter_json,
                    graphs_json = v.graphs_json,
                    status = v.status,
                    created_by_kind = v.created_by_kind
                FROM note_version AS v
                WHERE v.note_id = n.id
                  AND v.is_current;
                "#,
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Note::Table)
                    .modify_column(ColumnDef::new(Note::Title).text().not_null())
                    .modify_column(ColumnDef::new(Note::BodyMd).text().not_null())
                    .modify_column(
                        ColumnDef::new(Note::FrontmatterJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'::jsonb")),
                    )
                    .modify_column(
                        ColumnDef::new(Note::GraphsJson)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .modify_column(
                        ColumnDef::new(Note::Status)
                            .text()
                            .not_null()
                            .default("unread"),
                    )
                    .modify_column(ColumnDef::new(Note::CreatedByKind).text().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE note ADD CONSTRAINT note_status_check CHECK (status IN ('approved', 'unread', 'rejected')); \
                 ALTER TABLE note ADD CONSTRAINT note_created_by_kind_check CHECK (created_by_kind IN ('human', 'llm'));",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(NoteVersion::Table).to_owned())
            .await?;
        Ok(())
    }
}
