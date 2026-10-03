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
