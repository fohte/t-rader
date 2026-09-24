//! ノート本文・図から `[[kind:id]]` 参照を抽出し `note_ref` に同期する。
//!
//! REST の `/api/notes` handler と MCP `write_note` tool の両方から呼ばれる。

use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::note_ref;
use crate::error::AppError;
use crate::services::graph::GraphDef;

mod body_tokens;
use self::body_tokens::{
    BodyTokenPolicy, collect_note_refs, collect_note_refs_with_policy, format_note_token_errors,
};

/// note_ref を本文 + 図から都度 rebuild する: 旧 ref は DELETE で消え、
/// 本文または graphs[].ref に残るものだけ INSERT 復元する。
pub async fn sync_note_refs<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    sync_note_refs_with_policy(db, note_id, body_md, graphs_json, BodyTokenPolicy::Validate).await
}

pub(crate) async fn sync_note_refs_after_graphs_only_update<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    sync_note_refs_with_policy(
        db,
        note_id,
        body_md,
        graphs_json,
        BodyTokenPolicy::AllowLegacyBodyTokens,
    )
    .await
}

async fn sync_note_refs_with_policy<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
    body_token_policy: BodyTokenPolicy,
) -> Result<(), AppError> {
    let graphs = deserialize_graphs(graphs_json)?;
    let refs_result = match body_token_policy {
        BodyTokenPolicy::Validate => collect_note_refs(body_md, &graphs),
        BodyTokenPolicy::AllowLegacyBodyTokens => {
            collect_note_refs_with_policy(body_md, &graphs, body_token_policy)
        }
    };
    let mut refs =
        refs_result.map_err(|errors| AppError::Validation(format_note_token_errors(&errors)))?;
    refs.sort();
    refs.dedup();

    note_ref::Entity::delete_many()
        .filter(note_ref::Column::NoteId.eq(note_id))
        .exec(db)
        .await?;

    if refs.is_empty() {
        return Ok(());
    }

    let models: Vec<note_ref::ActiveModel> = refs
        .into_iter()
        .map(|(kind, id)| note_ref::ActiveModel {
            note_id: Set(note_id),
            ref_kind: Set(kind),
            ref_id: Set(id),
        })
        .collect();

    note_ref::Entity::insert_many(models)
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                note_ref::Column::NoteId,
                note_ref::Column::RefKind,
                note_ref::Column::RefId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(())
}

fn deserialize_graphs(graphs_json: &serde_json::Value) -> Result<Vec<GraphDef>, AppError> {
    serde_json::from_value(graphs_json.clone())
        .map_err(|e| AppError::Validation(format!("invalid graphs_json: {e}")))
}

#[cfg(test)]
mod tests {
    use sea_orm::{EntityTrait, QueryFilter};

    use super::*;

    #[test]
    fn test_deserialize_graphs_rejects_malformed_graphs_json() {
        let got = deserialize_graphs(&serde_json::json!([{"id": "g1"}])).unwrap_err();
        assert_eq!(
            got.to_string(),
            "validation error: invalid graphs_json: missing field `layout`",
        );
    }

    #[sqlx::test(migrations = false)]
    async fn sync_note_refs_indexes_refs_from_body_and_graphs_without_duplication(
        pool: sqlx::PgPool,
    ) {
        let db = crate::testing::create_test_db(pool).await;
        let strategy_id = crate::testing::insert_test_strategy(&db, "s").await;
        let note_id = crate::testing::insert_test_note(&db, strategy_id, "t", "orig").await;

        let graphs_json = serde_json::json!([{
            "id": "g1",
            "layout": "flow",
            "title": null,
            "nodes": [
                {
                    "id": "n1", "label": "node", "ref": "stock:demo-code",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
                {
                    "id": "n2", "label": "node", "ref": "stock:demo-code",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
            ],
            "edges": [],
        }]);

        sync_note_refs(
            &db,
            note_id,
            "body mentions [[stock:demo-code]] and [[theme:demo-theme]]",
            &graphs_json,
        )
        .await
        .unwrap();

        let mut refs = note_ref::Entity::find()
            .filter(note_ref::Column::NoteId.eq(note_id))
            .all(&db)
            .await
            .unwrap();
        refs.sort_by(|a, b| (&a.ref_kind, &a.ref_id).cmp(&(&b.ref_kind, &b.ref_id)));

        assert_eq!(
            refs.into_iter()
                .map(|r| (r.ref_kind, r.ref_id))
                .collect::<Vec<_>>(),
            vec![
                ("stock".to_string(), "demo-code".to_string()),
                ("theme".to_string(), "demo-theme".to_string()),
            ],
        );
    }
}
