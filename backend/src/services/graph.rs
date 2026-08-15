//! ノートに埋め込むグラフィカル表現 (`note.graphs_json`) のスキーマと検証。
//!
//! `#[serde(deny_unknown_fields)]` により、フィールド名の typo は
//! `missing field` ではなく `unknown field ..., expected one of ...` として
//! 返るため LLM が自力で直せる。
//!
//! `GraphDef` は schemars (`JsonSchema`) と utoipa (`ToSchema`) の両方を derive する。
//! 前者は rmcp が生成する MCP tool の input schema、後者は openapi-typescript
//! 経由の frontend 型の源になる。

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphDef {
    pub id: String,
    pub layout: Layout,
    pub title: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Flow,
    Tree,
    Chain,
    Scatter,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    /// 一級参照型トークン (例: "stock:7203" / "theme:weak-jpy")
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
    /// ノードサイズ / 棒の高さ。指定するなら `cite` も必須
    pub value: Option<f64>,
    /// `value` の出所 (自由テキスト)
    pub cite: Option<String>,
    /// grouping 先ノードの id
    pub parent: Option<String>,
    /// `layout = scatter` のときだけ使う指標値 (px ではない)
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    /// 線の太さ。指定するなら `cite` も必須
    pub value: Option<f64>,
    /// `value` の出所 (自由テキスト)
    pub cite: Option<String>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GraphValidationError {
    #[error("graph {graph_id:?}: {location} = {value:?} is not a known node id (known: {known})")]
    UnknownNodeId {
        graph_id: String,
        location: String,
        value: String,
        known: String,
    },
    #[error("graph {graph_id:?}: nodes[{index}].id = {value:?} is duplicated")]
    DuplicateNodeId {
        graph_id: String,
        index: usize,
        value: String,
    },
    #[error("graphs[{index}].id = {value:?} is duplicated")]
    DuplicateGraphId { index: usize, value: String },
    #[error(
        "graph {graph_id:?}: {location}.value is set but cite is missing (add cite noting the source)"
    )]
    MissingCite { graph_id: String, location: String },
}

/// `graphs` 全体を検証する。`graphs[].id` の重複禁止、参照整合性 (`nodes[].id` の
/// 重複禁止、`nodes[].parent` / `edges[].source` / `edges[].target` が `nodes[].id`
/// に存在するか) と、`value` があるのに `cite` が無い場合を拒否する。
pub fn validate_graphs(graphs: &[GraphDef]) -> Result<(), GraphValidationError> {
    let mut known_graph_ids: BTreeSet<&str> = BTreeSet::new();
    for (i, g) in graphs.iter().enumerate() {
        if !known_graph_ids.insert(g.id.as_str()) {
            return Err(GraphValidationError::DuplicateGraphId {
                index: i,
                value: g.id.clone(),
            });
        }
    }
    graphs.iter().try_for_each(validate_graph)
}

fn validate_graph(g: &GraphDef) -> Result<(), GraphValidationError> {
    let mut known_ids: BTreeSet<&str> = BTreeSet::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if !known_ids.insert(n.id.as_str()) {
            return Err(GraphValidationError::DuplicateNodeId {
                graph_id: g.id.clone(),
                index: i,
                value: n.id.clone(),
            });
        }
        if n.value.is_some() && n.cite.is_none() {
            return Err(GraphValidationError::MissingCite {
                graph_id: g.id.clone(),
                location: format!("nodes[{i}]"),
            });
        }
    }

    for (i, n) in g.nodes.iter().enumerate() {
        if let Some(parent) = n.parent.as_deref() {
            check_known_node_id(&g.id, format!("nodes[{i}].parent"), parent, &known_ids)?;
        }
    }

    for (i, e) in g.edges.iter().enumerate() {
        check_known_node_id(&g.id, format!("edges[{i}].source"), &e.source, &known_ids)?;
        check_known_node_id(&g.id, format!("edges[{i}].target"), &e.target, &known_ids)?;
        if e.value.is_some() && e.cite.is_none() {
            return Err(GraphValidationError::MissingCite {
                graph_id: g.id.clone(),
                location: format!("edges[{i}]"),
            });
        }
    }

    Ok(())
}

fn check_known_node_id(
    graph_id: &str,
    location: String,
    value: &str,
    known_ids: &BTreeSet<&str>,
) -> Result<(), GraphValidationError> {
    if known_ids.contains(value) {
        return Ok(());
    }
    Err(GraphValidationError::UnknownNodeId {
        graph_id: graph_id.to_string(),
        location,
        value: value.to_string(),
        known: known_ids.iter().copied().collect::<Vec<_>>().join(", "),
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            r#ref: None,
            value: None,
            cite: None,
            parent: None,
            x: None,
            y: None,
        }
    }

    fn edge(source: &str, target: &str) -> GraphEdge {
        GraphEdge {
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            value: None,
            cite: None,
        }
    }

    fn graph(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> GraphDef {
        GraphDef {
            id: "g1".to_string(),
            layout: Layout::Flow,
            title: None,
            nodes,
            edges,
        }
    }

    #[rstest]
    fn test_deserialize_full_graph() {
        let json = r#"{
            "id": "g1",
            "layout": "flow",
            "title": "半導体サプライチェーン",
            "nodes": [
                {"id": "asml", "label": "ASML", "ref": "stock:ASML", "value": 1.0, "cite": "10-K p.4", "parent": null, "x": null, "y": null},
                {"id": "tsmc", "label": "TSMC"}
            ],
            "edges": [
                {"source": "asml", "target": "tsmc", "label": "露光装置", "value": 0.3, "cite": "eval_indicator: revenue_share"}
            ]
        }"#;

        assert_eq!(
            serde_json::from_str::<GraphDef>(json).unwrap(),
            GraphDef {
                id: "g1".to_string(),
                layout: Layout::Flow,
                title: Some("半導体サプライチェーン".to_string()),
                nodes: vec![
                    GraphNode {
                        id: "asml".to_string(),
                        label: "ASML".to_string(),
                        r#ref: Some("stock:ASML".to_string()),
                        value: Some(1.0),
                        cite: Some("10-K p.4".to_string()),
                        parent: None,
                        x: None,
                        y: None,
                    },
                    GraphNode {
                        label: "TSMC".to_string(),
                        ..node("tsmc")
                    },
                ],
                edges: vec![GraphEdge {
                    source: "asml".to_string(),
                    target: "tsmc".to_string(),
                    label: Some("露光装置".to_string()),
                    value: Some(0.3),
                    cite: Some("eval_indicator: revenue_share".to_string()),
                }],
            },
        );
    }

    #[rstest]
    fn test_deserialize_rejects_unknown_field() {
        let json = r#"{"source": "asml", "target": "tsmc", "souce": "asml"}"#;
        let err = serde_json::from_str::<GraphEdge>(json).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown field `souce`, expected one of `source`, `target`, `label`, `value`, \
             `cite` at line 1 column 44",
        );
    }

    #[rstest]
    fn test_deserialize_rejects_unknown_layout_variant() {
        let json = r#"{"id": "g1", "layout": "sankey", "title": null, "nodes": [], "edges": []}"#;
        let err = serde_json::from_str::<GraphDef>(json).unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown variant `sankey`, expected one of `flow`, `tree`, `chain`, `scatter` at \
             line 1 column 31",
        );
    }

    #[rstest]
    fn test_validate_graphs_accepts_valid_graph() {
        let g = graph(vec![node("asml"), node("tsmc")], vec![edge("asml", "tsmc")]);
        assert_eq!(validate_graphs(&[g]), Ok(()));
    }

    #[rstest]
    #[case::source("tsmc2", "asml", "edges[0].source")]
    #[case::target("asml", "tsmc2", "edges[0].target")]
    fn test_validate_graphs_rejects_unknown_node_id(
        #[case] source: &str,
        #[case] target: &str,
        #[case] location: &str,
    ) {
        let g = graph(
            vec![node("asml"), node("tel"), node("tsmc")],
            vec![edge(source, target)],
        );

        assert_eq!(
            validate_graphs(&[g]),
            Err(GraphValidationError::UnknownNodeId {
                graph_id: "g1".to_string(),
                location: location.to_string(),
                value: "tsmc2".to_string(),
                known: "asml, tel, tsmc".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_rejects_duplicate_node_id() {
        let g = graph(vec![node("asml"), node("tel"), node("asml")], vec![]);

        assert_eq!(
            validate_graphs(&[g]),
            Err(GraphValidationError::DuplicateNodeId {
                graph_id: "g1".to_string(),
                index: 2,
                value: "asml".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_rejects_duplicate_graph_id() {
        let mut g2 = graph(vec![node("tel")], vec![]);
        g2.id = "g1".to_string();
        let g1 = graph(vec![node("asml")], vec![]);

        assert_eq!(
            validate_graphs(&[g1, g2]),
            Err(GraphValidationError::DuplicateGraphId {
                index: 1,
                value: "g1".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_rejects_unknown_parent() {
        let mut n = node("asml");
        n.parent = Some("does-not-exist".to_string());
        let g = graph(vec![n, node("tel")], vec![]);

        assert_eq!(
            validate_graphs(&[g]),
            Err(GraphValidationError::UnknownNodeId {
                graph_id: "g1".to_string(),
                location: "nodes[0].parent".to_string(),
                value: "does-not-exist".to_string(),
                known: "asml, tel".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_accepts_known_parent() {
        let mut n = node("asml");
        n.parent = Some("tel".to_string());
        let g = graph(vec![n, node("tel")], vec![]);

        assert_eq!(validate_graphs(&[g]), Ok(()));
    }

    #[rstest]
    fn test_validate_graphs_unknown_node_id_message_matches_design() {
        let g = graph(
            vec![node("asml"), node("tel"), node("tsmc")],
            vec![edge("asml", "tsmc2")],
        );

        assert_eq!(
            validate_graphs(&[g]).unwrap_err().to_string(),
            r#"graph "g1": edges[0].target = "tsmc2" is not a known node id (known: asml, tel, tsmc)"#,
        );
    }

    #[rstest]
    fn test_validate_graphs_rejects_node_value_without_cite() {
        let mut n = node("asml");
        n.value = Some(1.0);
        let g = graph(vec![n], vec![]);

        assert_eq!(
            validate_graphs(&[g]),
            Err(GraphValidationError::MissingCite {
                graph_id: "g1".to_string(),
                location: "nodes[0]".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_rejects_edge_value_without_cite() {
        let mut e = edge("asml", "tsmc");
        e.value = Some(0.5);
        let g = graph(vec![node("asml"), node("tsmc")], vec![e]);

        assert_eq!(
            validate_graphs(&[g]),
            Err(GraphValidationError::MissingCite {
                graph_id: "g1".to_string(),
                location: "edges[0]".to_string(),
            }),
        );
    }

    #[rstest]
    fn test_validate_graphs_accepts_value_with_cite() {
        let mut n = node("asml");
        n.value = Some(1.0);
        n.cite = Some("10-K p.4".to_string());
        let mut e = edge("asml", "tsmc");
        e.value = Some(0.5);
        e.cite = Some("eval_indicator: revenue_share".to_string());
        let g = graph(vec![n, node("tsmc")], vec![e]);

        assert_eq!(validate_graphs(&[g]), Ok(()));
    }
}
