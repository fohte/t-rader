import type { GraphDef, GraphEdge, GraphNode } from '#components/graph/types'
import type { components } from '#lib/api/schema.gen'

type ApiGraphDef = components['schemas']['GraphDef']

// backend は Option<T> を持つフィールドを `T | null` として返す。
// GraphRenderer が期待する型 (graph/types.ts) は `T | undefined` なので、ここで正規化する。
function toGraphNode(n: components['schemas']['GraphNode']): GraphNode {
  return {
    id: n.id,
    label: n.label,
    ref: n.ref ?? undefined,
    value: n.value ?? undefined,
    cite: n.cite ?? undefined,
    parent: n.parent ?? undefined,
    x: n.x ?? undefined,
    y: n.y ?? undefined,
  }
}

function toGraphEdge(e: components['schemas']['GraphEdge']): GraphEdge {
  return {
    source: e.source,
    target: e.target,
    label: e.label ?? undefined,
    value: e.value ?? undefined,
    cite: e.cite ?? undefined,
  }
}

export function toGraphDef(g: ApiGraphDef): GraphDef {
  return {
    id: g.id,
    layout: g.layout,
    title: g.title ?? undefined,
    nodes: g.nodes.map(toGraphNode),
    edges: g.edges.map(toGraphEdge),
  }
}
