#[cfg(test)]
use sea_orm::ActiveValue::Set;
#[cfg(test)]
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[cfg(test)]
use crate::error::AppError;
#[cfg(test)]
use crate::services::graph::GraphDef;
#[cfg(test)]
use gateway_postgres::entities::note_ref;

pub(crate) use core_domain::note_reference::ALLOWED_REF_KINDS;
#[cfg(test)]
pub(crate) use core_domain::note_reference::extract_note_link_tokens;
#[cfg(test)]
pub(crate) use core_domain::note_reference::{
    BodyTokenPolicy, NoteTokenValidationError, format_note_token_errors,
};

#[cfg(test)]
pub(crate) async fn sync_note_refs<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
) -> Result<(), AppError> {
    sync_note_refs_with_policy(db, note_id, body_md, graphs_json, BodyTokenPolicy::Validate).await
}

#[cfg(test)]
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

#[cfg(test)]
async fn sync_note_refs_with_policy<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: uuid::Uuid,
    body_md: &str,
    graphs_json: &serde_json::Value,
    body_token_policy: BodyTokenPolicy,
) -> Result<(), AppError> {
    let graphs = deserialize_graphs(graphs_json)?;
    let refs_result = collect_note_refs_with_policy(body_md, &graphs, body_token_policy);
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

#[cfg(test)]
fn deserialize_graphs(graphs_json: &serde_json::Value) -> Result<Vec<GraphDef>, AppError> {
    serde_json::from_value(graphs_json.clone())
        .map_err(|error| AppError::Validation(format!("invalid graphs_json: {error}")))
}

#[cfg(test)]
fn domain_graphs(graphs: &[GraphDef]) -> Vec<core_domain::note_graph::GraphDef> {
    graphs.iter().cloned().map(Into::into).collect()
}

#[cfg(test)]
fn collect_note_refs(
    body: &str,
    graphs: &[GraphDef],
) -> Result<Vec<(String, String)>, Vec<NoteTokenValidationError>> {
    core_domain::note_reference::collect_note_refs(body, &domain_graphs(graphs))
}

#[cfg(test)]
fn collect_note_refs_with_policy(
    body: &str,
    graphs: &[GraphDef],
    body_token_policy: BodyTokenPolicy,
) -> Result<Vec<(String, String)>, Vec<NoteTokenValidationError>> {
    core_domain::note_reference::collect_note_refs_with_policy(
        body,
        &domain_graphs(graphs),
        body_token_policy,
    )
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use core_domain::note_reference::extract_tokens;

    use super::*;
    use crate::services::graph::{GraphNode, Layout};

    fn graph(id: &str, node_ref: Option<&str>) -> GraphDef {
        GraphDef {
            id: id.to_string(),
            layout: Layout::Flow,
            title: None,
            nodes: vec![GraphNode {
                id: "node-1".to_string(),
                label: "node".to_string(),
                r#ref: node_ref.map(str::to_string),
                value: None,
                cite: None,
                parent: None,
                x: None,
                y: None,
            }],
            edges: Vec::new(),
        }
    }

    #[rstest]
    #[case::array_literal("[[1, 2], [3, 4]]", vec![])]
    #[case::malformed_before_valid("[[bad] then [[stock:demo-code]]", vec!["[[stock:demo-code]]"])]
    #[case::adjacent("[[stock:demo-code]][[theme:demo-theme]]", vec!["[[stock:demo-code]]", "[[theme:demo-theme]]"])]
    #[case::unclosed("[[unfinished", vec![])]
    #[case::empty_inner("[[]]", vec![])]
    #[case::emptyish_inner("[[ ]]", vec!["[[ ]]"])]
    #[case::newline_inside("[[stock:\ndemo-code]]", vec!["[[stock:\ndemo-code]]"])]
    fn test_extract_tokens_matches_frontend_shape(#[case] body: &str, #[case] expected: Vec<&str>) {
        let got = extract_tokens(body)
            .iter()
            .map(|token| &body[token.start..token.end])
            .collect::<Vec<_>>();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_collect_note_refs_accepts_reference_annotation_graph_and_array_literal() {
        let body = concat!(
            "[[stock:demo-code]] [[indicator:demo-index]] [[sector:demo-sector]] [[theme:demo-theme]] ",
            "[[anno:a-1]]\n\n [[graph:g1]] \n\n[[1, 2], [3, 4]]",
        );
        assert_eq!(
            collect_note_refs(body, &[graph("g1", Some("stock:demo-code"))]),
            Ok(vec![
                ("stock".to_string(), "demo-code".to_string()),
                ("indicator".to_string(), "demo-index".to_string()),
                ("sector".to_string(), "demo-sector".to_string()),
                ("theme".to_string(), "demo-theme".to_string()),
                ("stock".to_string(), "demo-code".to_string()),
            ]),
        );
    }

    #[rstest]
    #[case::inline_code("inline `[[foo:bar]]` code")]
    #[case::multiline_inline_code("`[[foo:bar]]\n[[stock:demo-code]]`")]
    #[case::backtick_fence(indoc::indoc! {"
        ```text
        [[foo:bar]]
        ```
    "})]
    #[case::tilde_fence(indoc::indoc! {"
        ~~~text
        [[graph:g1]]
        ~~~
    "})]
    #[case::indented_code("    [[foo:bar]]")]
    fn test_collect_note_refs_ignores_markdown_code(#[case] body: &str) {
        assert_eq!(collect_note_refs(body, &[]), Ok(vec![]));
    }

    #[test]
    fn test_collect_note_refs_does_not_extend_inline_code_across_blank_lines() {
        let body = indoc::indoc! {"
            `unclosed

            [[foo:bar]]
        "};
        assert_eq!(
            collect_note_refs(body, &[]).map_err(|_| "invalid"),
            Err("invalid")
        );
    }

    #[test]
    fn test_collect_note_refs_reports_every_invalid_token_and_allowed_form() {
        let body = indoc::indoc! {"
            [[foo:bar]] [[bare-demo]] [[stock:]] [[anno:]] [[graph:1bad]]

            [[graph:missing]]

            mixed [[graph:g1]]
        "};
        let errors = collect_note_refs(body, &[graph("g1", Some("foo:bar"))])
            .expect_err("invalid note tokens should be rejected");
        assert_eq!(
            format_note_token_errors(&errors),
            concat!(
                "ノートのトークンに問題があります:\n",
                "- 本文のトークン \"[[foo:bar]]\": 未知の prefix `foo` です\n",
                "- 本文のトークン \"[[bare-demo]]\": kind:id の形式で prefix を指定してください\n",
                "- 本文のトークン \"[[stock:]]\": 参照 ID を空にできません\n",
                "- 本文のトークン \"[[anno:]]\": annotation ID は英数字で始まり、英数字・`_`・`-` のみ使用できます\n",
                "- 本文のトークン \"[[graph:1bad]]\": graph ID は英字で始まり、英数字・`_`・`-` のみ使用できます\n",
                "- 本文のトークン \"[[graph:missing]]\": 対応する graphs[].id がありません\n",
                "- 本文のトークン \"[[graph:g1]]\": 図トークンは空行区切りブロック内で単独にしてください\n",
                "- graphs[0].nodes[0].ref の値 \"[[foo:bar]]\": 未知の prefix `foo` です; 図ノードでは stock / indicator / sector / theme の参照だけを使用できます\n",
                "許可される形式: `[[stock:<id>]]`, `[[indicator:<id>]]`, `[[sector:<id>]]`, `[[theme:<id>]]`, `[[note:<uuid>]]`, `[[note:<uuid>@current]]`, `[[anno:<id>]]`。`[[graph:<id>]]` は graphs[].id に存在し、空行区切りブロック内で単独にしてください。graphs[].nodes[].ref では参照 4 種のみ使用できます。",
            ),
        );
    }

    #[test]
    fn test_deserialize_graphs_rejects_malformed_graphs_json() {
        let got = deserialize_graphs(&serde_json::json!([{"id": "g1"}])).unwrap_err();
        assert_eq!(
            got.to_string(),
            "validation error: invalid graphs_json: missing field `layout`",
        );
    }

    #[rstest]
    #[case::mixed_paragraph("text [[graph:g1]]", 1)]
    #[case::same_block_multiple("[[graph:g1]]\n[[graph:g1]]", 2)]
    fn test_collect_note_refs_rejects_non_standalone_graph_tokens(
        #[case] body: &str,
        #[case] error_count: usize,
    ) {
        assert_eq!(
            collect_note_refs(body, &[graph("g1", None)]),
            Err(vec![
                NoteTokenValidationError::GraphToken {
                    token: "[[graph:g1]]".to_string(),
                    reason: "図トークンは空行区切りブロック内で単独にしてください".to_string(),
                };
                error_count
            ]),
        );
    }

    #[rstest]
    #[case::with_ref(
        graph("g1", Some("stock:demo-code")),
        Ok(vec![("stock".to_string(), "demo-code".to_string())])
    )]
    #[case::no_ref(graph("g1", None), Ok(vec![]))]
    #[case::invalid_kind(graph("g1", Some("foo:bar")), Err("invalid"))]
    #[case::annotation_not_allowed(graph("g1", Some("anno:a1")), Err("invalid"))]
    fn test_collect_note_refs_validates_graph_node_refs(
        #[case] graph: GraphDef,
        #[case] expected: Result<Vec<(String, String)>, &str>,
    ) {
        let got = collect_note_refs("", &[graph]);
        assert_eq!(got.map_err(|_| "invalid"), expected);
    }

    #[backend_test_macros::database_test]
    async fn sync_note_refs_indexes_refs_from_body_and_graphs_without_duplication(
        db: gateway_postgres::DatabaseHandle,
    ) {
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
                .map(|reference| (reference.ref_kind, reference.ref_id))
                .collect::<Vec<_>>(),
            vec![
                ("stock".to_string(), "demo-code".to_string()),
                ("theme".to_string(), "demo-theme".to_string()),
            ],
        );
    }
}
