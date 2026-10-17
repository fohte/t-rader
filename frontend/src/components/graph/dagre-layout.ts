import dagre, {
  type EdgeLabel,
  type GraphLabel,
  graphlib,
  type NodeLabel,
} from '@dagrejs/dagre'
import { err, ok, Result } from 'neverthrow'

import type { GraphFlowEdge, GraphFlowNode } from '#components/graph/flow-types'
import {
  buildFlowEdges,
  estimateEdgeLabelSize,
  nodeHeight,
  nodeWidth,
  validateGraphRefs,
} from '#components/graph/graph-utils'
import type { GraphDef } from '#components/graph/types'

type DagreGraph = graphlib.Graph<GraphLabel, NodeLabel, EdgeLabel>

// dagre のノード中心 + 絶対座標を左上基準に変換する。group ノードは width/height を
// 明示せずに登録しても、子ノードの配置から dagre が layout 後に自動計算して埋める。
function topLeft(g: DagreGraph, id: string) {
  const n = g.node(id)
  return {
    x: (n.x ?? 0) - n.width / 2,
    y: (n.y ?? 0) - n.height / 2,
    width: n.width,
    height: n.height,
  }
}

export function buildDagreLayout(
  def: GraphDef,
  rankdir: 'LR' | 'TB',
  citeNumbers: Map<string, number>,
): Result<{ nodes: GraphFlowNode[]; edges: GraphFlowEdge[] }, Error> {
  const validation = validateGraphRefs(def)
  if (validation.isErr()) return err(validation.error)

  // スキーマに group フラグは無い。誰かの parent として参照されていれば group とみなす
  const groupIds = new Set(
    def.nodes.map((n) => n.parent).filter((p): p is string => p != null),
  )
  const edges = buildFlowEdges(def.edges, citeNumbers)

  const g: DagreGraph = new dagre.graphlib.Graph({
    compound: true,
    multigraph: true,
  })
  g.setDefaultEdgeLabel(() => ({}))
  g.setGraph({ rankdir, nodesep: 40, ranksep: 90 })

  // dagre は循環する parent 参照 (LLM が書く JSON にネスト深さ・循環の制約は無い) を
  // 検出すると同期的に throw する。Result に変換して呼び出し元のエラー表示に載せる
  const runDagre = Result.fromThrowable(
    () => {
      for (const node of def.nodes) {
        const isGroup = groupIds.has(node.id)
        g.setNode(
          node.id,
          isGroup
            ? { width: 0, height: 0 }
            : { width: nodeWidth(node), height: nodeHeight(node) },
        )
        if (node.parent != null) g.setParent(node.id, node.parent)
      }
      for (const [index, edge] of edges.entries()) {
        const labelSize =
          typeof edge.label !== 'string' || edge.label === ''
            ? {}
            : estimateEdgeLabelSize(edge.label)
        g.setEdge(edge.source, edge.target, labelSize, String(index))
      }
      dagre.layout(g)
    },
    (e) => (e instanceof Error ? e : new Error(String(e))),
  )
  const dagreResult = runDagre()
  if (dagreResult.isErr()) return err(dagreResult.error)

  // React Flow は親ノードが子より配列内で前に来ることを要求する。LLM が書く JSON の
  // 順序に依存しないようここでソートする。
  const sortedNodes = [...def.nodes].sort(
    (a, b) => Number(a.parent != null) - Number(b.parent != null),
  )

  const nodes: GraphFlowNode[] = sortedNodes.map((node) => {
    const isGroup = groupIds.has(node.id)
    const abs = topLeft(g, node.id)
    const position =
      node.parent != null
        ? {
            x: abs.x - topLeft(g, node.parent).x,
            y: abs.y - topLeft(g, node.parent).y,
          }
        : { x: abs.x, y: abs.y }

    return {
      id: node.id,
      type: isGroup ? 'graphGroup' : 'graphNode',
      position,
      parentId: node.parent,
      extent: node.parent != null ? 'parent' : undefined,
      data: node,
      // group ノードは React Flow が親ノードの自動サイズ調整をしないため、明示が必要
      ...(isGroup ? { style: { width: abs.width, height: abs.height } } : {}),
    }
  })

  const edgesWithLayout: GraphFlowEdge[] = edges.map((edge, index) => {
    const dagreEdge = g.edge({
      v: edge.source,
      w: edge.target,
      name: String(index),
    })
    return {
      ...edge,
      data: {
        ...def.edges[index],
        dagre: {
          points: dagreEdge.points ?? [],
          ...(dagreEdge.x == null || dagreEdge.y == null
            ? {}
            : { labelPosition: { x: dagreEdge.x, y: dagreEdge.y } }),
        },
      },
    }
  })

  return ok({ nodes, edges: edgesWithLayout })
}
