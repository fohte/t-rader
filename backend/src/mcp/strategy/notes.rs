//! ノート操作の inner method 実装。
//!
//! 戦略境界の二重検査は [`super::ensure_strategy_match`] と
//! [`super::fetch_note_owned_by`] が担う。

use rmcp::ErrorData as McpError;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::note;

use super::dto::{
    ListNotesParams, ListNotesResult, NoteDto, ReadNoteParams, WriteNoteParams, WriteNoteResult,
};
use super::{
    DEFAULT_NOTE_STATUS, STRATEGY_AGENT_ACTOR, StrategyServer, clamp_limit, db_error,
    ensure_strategy_exists, ensure_strategy_match, fetch_note_owned_by, invalid_params,
};

fn ensure_frontmatter_object(fm: &serde_json::Value) -> Result<(), McpError> {
    if fm.is_object() {
        Ok(())
    } else {
        Err(invalid_params("frontmatter_json must be a JSON object"))
    }
}

fn note_to_dto(m: note::Model) -> NoteDto {
    NoteDto {
        note_id: m.id,
        strategy_id: m.strategy_id,
        title: m.title,
        body_md: m.body_md,
        frontmatter_json: m.frontmatter_json,
        type_tag: m.type_tag,
        status: m.status,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

impl StrategyServer {
    pub(crate) async fn write_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        if let Some(fm) = params.frontmatter_json.as_ref() {
            ensure_frontmatter_object(fm)?;
        }

        if let Some(note_id) = params.note_id {
            let current = fetch_note_owned_by(&self.db, note_id, params.strategy_id).await?;
            let mut active = current.clone().into_active_model();
            let mut touched = false;
            if let Some(title) = params.title {
                let title = title.trim().to_string();
                if title.is_empty() {
                    return Err(invalid_params("title must not be empty"));
                }
                active.title = Set(title);
                touched = true;
            }
            if let Some(body) = params.body_md {
                active.body_md = Set(body);
                touched = true;
            }
            if let Some(tag) = params.type_tag {
                active.type_tag = Set(tag);
                touched = true;
            }
            if let Some(fm) = params.frontmatter_json {
                active.frontmatter_json = Set(fm);
                touched = true;
            }
            if !touched {
                return Err(invalid_params(
                    "at least one of title / body_md / type_tag / frontmatter_json must be provided",
                ));
            }
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            active.update(&self.db).await.map_err(db_error)?;
            return Ok(WriteNoteResult {
                note_id,
                created: false,
            });
        }

        ensure_strategy_exists(&self.db, params.strategy_id).await?;
        let title = params
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid_params("title is required when creating a new note"))?
            .to_string();
        let body_md = params.body_md.unwrap_or_default();
        let frontmatter_json = params
            .frontmatter_json
            .unwrap_or_else(|| serde_json::json!({}));
        let id = Uuid::new_v4();
        let model = note::ActiveModel {
            id: Set(id),
            strategy_id: Set(params.strategy_id),
            title: Set(title),
            body_md: Set(body_md),
            frontmatter_json: Set(frontmatter_json),
            type_tag: Set(params.type_tag.flatten()),
            status: Set(DEFAULT_NOTE_STATUS.to_string()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        note::Entity::insert(model)
            .exec_without_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(WriteNoteResult {
            note_id: id,
            created: true,
        })
    }

    pub(crate) async fn read_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let row = fetch_note_owned_by(&self.db, params.note_id, params.strategy_id).await?;
        Ok(note_to_dto(row))
    }

    pub(crate) async fn list_notes_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let rows = note::Entity::find()
            .filter(note::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(note::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ListNotesResult {
            notes: rows.into_iter().map(note_to_dto).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::testing::create_test_db;

    use super::super::dto::{ListNotesParams, ReadNoteParams, WriteNoteParams, WriteNoteResult};
    use super::super::tests_common::{
        build_server, insert_strategy, normalize_note, seed_foreign_note, ts_sentinel,
    };
    use super::super::{DEFAULT_NOTE_STATUS, NoteDto, STRATEGY_AGENT_ACTOR};

    #[sqlx::test(migrations = false)]
    async fn write_note_creates_then_read_note_returns_it(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let written = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: None,
                    title: Some("first note".into()),
                    body_md: Some("body".into()),
                    type_tag: Some(Some("observation".into())),
                    frontmatter_json: None,
                },
            )
            .await
            .expect("write_note");
        assert!(written.created);

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    strategy_id,
                    note_id: written.note_id,
                },
            )
            .await
            .expect("read_note");

        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: written.note_id,
                strategy_id,
                title: "first note".into(),
                body_md: "body".into(),
                frontmatter_json: serde_json::json!({}),
                type_tag: Some("observation".into()),
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_updates_existing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);

        let created = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: None,
                    title: Some("original".into()),
                    body_md: Some("v1".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect("create");

        let updated = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("v2".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect("update");
        assert_eq!(
            updated,
            WriteNoteResult {
                note_id: created.note_id,
                created: false,
            },
        );

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    strategy_id,
                    note_id: created.note_id,
                },
            )
            .await
            .expect("read");
        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: created.note_id,
                strategy_id,
                title: "original".into(),
                body_md: "v2".into(),
                frontmatter_json: serde_json::json!({}),
                type_tag: None,
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
            },
        );
    }

    /// `type_tag: Some(None)` (JSON で `"type_tag": null`) は既存タグの NULL クリアとして扱う
    #[sqlx::test(migrations = false)]
    async fn write_note_clears_type_tag_with_explicit_null(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);

        let created = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: None,
                    title: Some("t".into()),
                    body_md: None,
                    type_tag: Some(Some("observation".into())),
                    frontmatter_json: None,
                },
            )
            .await
            .expect("create");

        // タグを明示的に null へ更新
        server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: None,
                    type_tag: Some(None),
                    frontmatter_json: None,
                },
            )
            .await
            .expect("clear");

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    strategy_id,
                    note_id: created.note_id,
                },
            )
            .await
            .expect("read");
        assert_eq!(read.type_tag, None);
    }

    /// `Option<Option<String>>` のシリアライズ意味論を pin する。
    /// フィールド省略 → `None` (touch しない)、`null` 明示 → `Some(None)` (NULL クリア)、値あり → `Some(Some(v))`。
    #[test]
    fn write_note_params_type_tag_deserialization() {
        fn parse(json: &str) -> Option<Option<String>> {
            serde_json::from_str::<WriteNoteParams>(json)
                .expect("parse")
                .type_tag
        }
        let sid = r#""strategy_id":"00000000-0000-0000-0000-000000000000""#;
        assert_eq!(
            (
                parse(&format!("{{{sid}}}")),
                parse(&format!("{{{sid},\"type_tag\":null}}")),
                parse(&format!("{{{sid},\"type_tag\":\"observation\"}}")),
            ),
            (None, Some(None), Some(Some("observation".into()))),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_arg_mismatch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db);

        let err = server
            .write_note_inner(
                strategy_a,
                WriteNoteParams {
                    strategy_id: strategy_b,
                    note_id: None,
                    title: Some("x".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect_err("boundary violation expected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_cross_strategy_update(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        let err = server
            .write_note_inner(
                strategy_a,
                WriteNoteParams {
                    strategy_id: strategy_a,
                    note_id: Some(note_id),
                    title: None,
                    body_md: Some("hijack".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect_err("cross-strategy update expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_note_rejects_cross_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        let err = server
            .read_note_inner(
                strategy_a,
                ReadNoteParams {
                    strategy_id: strategy_a,
                    note_id,
                },
            )
            .await
            .expect_err("cross-strategy read expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_filters_by_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db);

        for (sid, title) in [(strategy_a, "a1"), (strategy_a, "a2"), (strategy_b, "b1")] {
            server
                .write_note_inner(
                    sid,
                    WriteNoteParams {
                        strategy_id: sid,
                        note_id: None,
                        title: Some(title.into()),
                        body_md: None,
                        type_tag: None,
                        frontmatter_json: None,
                    },
                )
                .await
                .expect("write");
        }

        let result = server
            .list_notes_inner(
                strategy_a,
                ListNotesParams {
                    strategy_id: strategy_a,
                    limit: None,
                },
            )
            .await
            .expect("list");
        // 戦略 B のノートは含まれず、戦略 A の 2 件のみが新しい順に並ぶ
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        let strategies: Vec<Uuid> = result.notes.iter().map(|n| n.strategy_id).collect();
        assert_eq!(
            (titles, strategies),
            (vec!["a2", "a1"], vec![strategy_a, strategy_a]),
        );
    }
}
