import dagre, {
  type EdgeLabel,
  type GraphLabel,
  graphlib,
  type NodeLabel,
} from '@dagrejs/dagre'
import { Position } from '@xyflow/react'
import { err, ok, type Result } from 'neverthrow'

import type { GraphFlowEdge, GraphFlowNode } from '#components/graph/flow-types'
import {
  buildFlowEdges,
  NODE_HEIGHT,
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

  const g: DagreGraph = new dagre.graphlib.Graph({ compound: true })
  g.setDefaultEdgeLabel(() => ({}))
  g.setGraph({ rankdir, nodesep: 40, ranksep: 90 })

  for (const node of def.nodes) {
    const isGroup = groupIds.has(node.id)
    g.setNode(
      node.id,
      isGroup
        ? { width: 0, height: 0 }
        : { width: nodeWidth(node), height: NODE_HEIGHT },
    )
    if (node.parent != null) g.setParent(node.id, node.parent)
  }
  for (const edge of def.edges) {
    g.setEdge(edge.source, edge.target)
  }

  dagre.layout(g)

  const sourcePosition = rankdir === 'LR' ? Position.Right : Position.Bottom
  const targetPosition = rankdir === 'LR' ? Position.Left : Position.Top

  // React Flow は親ノードが子より配列内で前に来ることを要求する。LLM が書く JSON の
  // 順序に依存しないようここでソートする。
  // ponytail: 1 階層のみ対応。グループのネストが要るなら深さ順の topological sort に拡張する
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
      sourcePosition,
      targetPosition,
      data: node,
      // group ノードは React Flow が親ノードの自動サイズ調整をしないため、明示が必要
      ...(isGroup ? { style: { width: abs.width, height: abs.height } } : {}),
    }
  })

  return ok({ nodes, edges: buildFlowEdges(def.edges, citeNumbers) })
}
