use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260924_171836_note_version"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE note_version (
                    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
                    note_id uuid NOT NULL REFERENCES note(id) ON DELETE CASCADE,
                    version_no integer NOT NULL,
                    title text NOT NULL,
                    body_md text NOT NULL,
                    frontmatter_json jsonb NOT NULL DEFAULT '{}'::jsonb,
                    graphs_json jsonb NOT NULL DEFAULT '[]'::jsonb,
                    status text NOT NULL DEFAULT 'unread'
                        CHECK (status IN ('approved', 'unread', 'rejected')),
                    is_current boolean NOT NULL DEFAULT false,
                    change_reason text,
                    created_by_kind text NOT NULL
                        CHECK (created_by_kind IN ('human', 'llm')),
                    execution_id text,
                    created_at timestamptz NOT NULL DEFAULT now(),
                    reviewed_at timestamptz,
                    UNIQUE (note_id, version_no)
                );

                CREATE UNIQUE INDEX idx_note_version_current
                    ON note_version (note_id)
                    WHERE is_current;

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

                ALTER TABLE note
                    DROP COLUMN title,
                    DROP COLUMN body_md,
                    DROP COLUMN frontmatter_json,
                    DROP COLUMN graphs_json,
                    DROP COLUMN status,
                    DROP COLUMN created_by_kind;
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE note
                    ADD COLUMN title text,
                    ADD COLUMN body_md text,
                    ADD COLUMN frontmatter_json jsonb DEFAULT '{}'::jsonb,
                    ADD COLUMN graphs_json jsonb DEFAULT '[]'::jsonb,
                    ADD COLUMN status text DEFAULT 'unread',
                    ADD COLUMN created_by_kind text;

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

                ALTER TABLE note
                    ALTER COLUMN title SET NOT NULL,
                    ALTER COLUMN body_md SET NOT NULL,
                    ALTER COLUMN frontmatter_json SET NOT NULL,
                    ALTER COLUMN graphs_json SET NOT NULL,
                    ALTER COLUMN status SET NOT NULL,
                    ALTER COLUMN created_by_kind SET NOT NULL,
                    ADD CONSTRAINT note_status_check
                        CHECK (status IN ('approved', 'unread', 'rejected')),
                    ADD CONSTRAINT note_created_by_kind_check
                        CHECK (created_by_kind IN ('human', 'llm'));

                DROP TABLE note_version;
                "#,
            )
            .await?;
        Ok(())
    }
}
