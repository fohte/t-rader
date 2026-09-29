use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDef {
    pub id: String,
    pub layout: Layout,
    pub title: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Flow,
    Tree,
    Chain,
    Scatter,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
    pub value: Option<f64>,
    pub cite: Option<String>,
    pub parent: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub value: Option<f64>,
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

pub fn validate_graphs(graphs: &[GraphDef]) -> Result<(), GraphValidationError> {
    let mut known_graph_ids: BTreeSet<&str> = BTreeSet::new();
    for (i, graph) in graphs.iter().enumerate() {
        if !known_graph_ids.insert(graph.id.as_str()) {
            return Err(GraphValidationError::DuplicateGraphId {
                index: i,
                value: graph.id.clone(),
            });
        }
    }
    graphs.iter().try_for_each(validate_graph)
}

fn validate_graph(graph: &GraphDef) -> Result<(), GraphValidationError> {
    let mut known_ids: BTreeSet<&str> = BTreeSet::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        if !known_ids.insert(node.id.as_str()) {
            return Err(GraphValidationError::DuplicateNodeId {
                graph_id: graph.id.clone(),
                index,
                value: node.id.clone(),
            });
        }
        if node.value.is_some() && node.cite.is_none() {
            return Err(GraphValidationError::MissingCite {
                graph_id: graph.id.clone(),
                location: format!("nodes[{index}]"),
            });
        }
    }

    for (index, node) in graph.nodes.iter().enumerate() {
        if let Some(parent) = node.parent.as_deref() {
            check_known_node_id(
                &graph.id,
                format!("nodes[{index}].parent"),
                parent,
                &known_ids,
            )?;
        }
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        check_known_node_id(
            &graph.id,
            format!("edges[{index}].source"),
            &edge.source,
            &known_ids,
        )?;
        check_known_node_id(
            &graph.id,
            format!("edges[{index}].target"),
            &edge.target,
            &known_ids,
        )?;
        if edge.value.is_some() && edge.cite.is_none() {
            return Err(GraphValidationError::MissingCite {
                graph_id: graph.id.clone(),
                location: format!("edges[{index}]"),
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
