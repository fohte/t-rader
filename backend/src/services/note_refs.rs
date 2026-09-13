//! ノート本文・図から `[[kind:id]]` 参照を抽出し `note_ref` に同期する。
//!
//! REST の `/api/notes` handler と MCP `write_note` tool の両方から呼ばれる。

use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::note_ref;
use crate::error::AppError;
use crate::services::graph::GraphDef;

const ALLOWED_REF_KINDS: [&str; 4] = ["stock", "indicator", "sector", "theme"];

/// note_ref を本文 + 図から都度 rebuild する: 旧 ref は DELETE で消え、
/// 本文または graphs[].ref に残るものだけ INSERT 復元する
pub async fn sync_note_refs<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    let mut refs = extract_refs(body_md);
    refs.extend(extract_graph_refs(graphs_json)?);
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

/// `"kind:id"` 形式の参照トークンを、`ALLOWED_REF_KINDS` に含まれ id が非空の場合のみ許可する
fn parse_ref_token(token: &str) -> Option<(String, String)> {
    let (kind, id) = token.split_once(':')?;
    let kind = kind.trim();
    let id = id.trim();
    (ALLOWED_REF_KINDS.contains(&kind) && !id.is_empty())
        .then(|| (kind.to_string(), id.to_string()))
}

fn extract_refs(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let inner = &rest[..end];
        if let Some(pair) = parse_ref_token(inner) {
            out.push(pair);
        }
        rest = &rest[end + 2..];
    }
    out
}

/// `nodes[].ref` を集める点は `extract_refs` と同じだが、デシリアライズ失敗は握りつぶさず `AppError` として伝播する
fn extract_graph_refs(graphs_json: &serde_json::Value) -> Result<Vec<(String, String)>, AppError> {
    let graphs: Vec<GraphDef> = serde_json::from_value(graphs_json.clone())
        .map_err(|e| AppError::Validation(format!("invalid graphs_json: {e}")))?;
    Ok(graphs
        .iter()
        .flat_map(|g| g.nodes.iter())
        .filter_map(|n| n.r#ref.as_deref())
        .filter_map(parse_ref_token)
        .collect())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use super::*;
    use crate::testing::{create_test_db, insert_test_note, insert_test_strategy};

    #[rstest]
    #[case::single("hello [[stock:7203]]", vec![("stock", "7203")])]
    #[case::multiple("a [[indicator:USDJPY]] b [[theme:weak-jpy]]", vec![("indicator", "USDJPY"), ("theme", "weak-jpy")])]
    #[case::unknown_kind_ignored("[[foo:bar]] [[stock:9984]]", vec![("stock", "9984")])]
    #[case::no_prefix_ignored("[[7203]]", vec![])]
    #[case::empty("", vec![])]
    fn test_extract_refs(#[case] body: &str, #[case] expected: Vec<(&str, &str)>) {
        let got = extract_refs(body);
        let got: Vec<(&str, &str)> = got.iter().map(|(k, i)| (k.as_str(), i.as_str())).collect();
        assert_eq!(got, expected);
    }

    fn graph_json_with_node_ref(node_ref: Value) -> Value {
        json!([{
            "id": "g1",
            "layout": "flow",
            "title": null,
            "nodes": [{
                "id": "n1",
                "label": "ASML",
                "ref": node_ref,
                "value": null,
                "cite": null,
                "parent": null,
                "x": null,
                "y": null,
            }],
            "edges": [],
        }])
    }

    #[rstest]
    #[case::with_ref(graph_json_with_node_ref(json!("stock:ASML")), vec![("stock", "ASML")])]
    #[case::no_ref(graph_json_with_node_ref(Value::Null), vec![])]
    #[case::unknown_kind_ignored(graph_json_with_node_ref(json!("foo:bar")), vec![])]
    #[case::empty(json!([]), vec![])]
    fn test_extract_graph_refs(#[case] graphs_json: Value, #[case] expected: Vec<(&str, &str)>) {
        let got = extract_graph_refs(&graphs_json).unwrap();
        let got: Vec<(&str, &str)> = got.iter().map(|(k, i)| (k.as_str(), i.as_str())).collect();
        assert_eq!(got, expected);
    }

    #[rstest]
    fn test_extract_graph_refs_rejects_malformed_graphs_json() {
        let got = extract_graph_refs(&json!([{"id": "g1"}])).unwrap_err();
        assert_eq!(
            got.to_string(),
            "validation error: invalid graphs_json: missing field `layout`",
        );
    }

    #[sqlx::test(migrations = false)]
    async fn sync_note_refs_indexes_refs_from_both_body_and_graphs_without_duplication(
        pool: PgPool,
    ) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_test_strategy(&db, "s").await;
        let note_id = insert_test_note(&db, strategy_id, "t", "orig").await;

        let graphs_json = json!([{
            "id": "g1",
            "layout": "flow",
            "title": null,
            "nodes": [
                {
                    "id": "n1", "label": "ASML", "ref": "stock:ASML",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
                {
                    "id": "n2", "label": "TSMC", "ref": "stock:7203",
                    "value": null, "cite": null, "parent": null, "x": null, "y": null,
                },
            ],
            "edges": [],
        }]);

        sync_note_refs(
            &db,
            note_id,
            "body mentions [[stock:7203]] and [[theme:weak-jpy]]",
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
                ("stock".to_string(), "7203".to_string()),
                ("stock".to_string(), "ASML".to_string()),
                ("theme".to_string(), "weak-jpy".to_string()),
            ],
        );
    }
}
