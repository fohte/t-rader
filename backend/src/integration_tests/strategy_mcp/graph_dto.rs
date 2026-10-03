use core_domain::note_graph as domain;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphDef {
    pub id: String,
    pub layout: Layout,
    pub title: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Flow,
    Tree,
    Chain,
    Scatter,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub value: Option<f64>,
    pub cite: Option<String>,
}

impl From<GraphDef> for domain::GraphDef {
    fn from(graph: GraphDef) -> Self {
        Self {
            id: graph.id,
            layout: graph.layout.into(),
            title: graph.title,
            nodes: graph.nodes.into_iter().map(Into::into).collect(),
            edges: graph.edges.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Layout> for domain::Layout {
    fn from(layout: Layout) -> Self {
        match layout {
            Layout::Flow => Self::Flow,
            Layout::Tree => Self::Tree,
            Layout::Chain => Self::Chain,
            Layout::Scatter => Self::Scatter,
        }
    }
}

impl From<GraphNode> for domain::GraphNode {
    fn from(node: GraphNode) -> Self {
        Self {
            id: node.id,
            label: node.label,
            r#ref: node.r#ref,
            value: node.value,
            cite: node.cite,
            parent: node.parent,
            x: node.x,
            y: node.y,
        }
    }
}

impl From<GraphEdge> for domain::GraphEdge {
    fn from(edge: GraphEdge) -> Self {
        Self {
            source: edge.source,
            target: edge.target,
            label: edge.label,
            value: edge.value,
            cite: edge.cite,
        }
    }
}
