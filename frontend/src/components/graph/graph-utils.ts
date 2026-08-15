import { Position } from '@xyflow/react'
import { err, ok, type Result } from 'neverthrow'

import type { GraphFlowEdge } from '#components/graph/flow-types'
import type {
  GraphDef,
  GraphEdge,
  GraphNode,
  Layout,
} from '#components/graph/types'

export const NODE_WIDTH_BASE = 160
export const NODE_HEIGHT = 56

const EDGE_STROKE_MIN = 1.5
const EDGE_STROKE_MAX = 5

/** value を持つノードは幅を少し広げる (例: value 60 → 208px) */
export function nodeWidth(node: GraphNode): number {
  if (typeof node.value !== 'number') return NODE_WIDTH_BASE
  return NODE_WIDTH_BASE + Math.min(node.value, 100) * 0.8
}

/** tree のみ縦方向 (Bottom→Top)、それ以外は横方向 (Right→Left) に Handle を置く */
export function handlePositions(layout: Layout): {
  source: Position
  target: Position
} {
  return layout === 'tree'
    ? { source: Position.Bottom, target: Position.Top }
    : { source: Position.Right, target: Position.Left }
}

export function edgeStrokeWidth(
  value: number | undefined,
  maxValue: number,
): number {
  if (typeof value !== 'number' || maxValue <= 0) return EDGE_STROKE_MIN
  return (
    EDGE_STROKE_MIN + (value / maxValue) * (EDGE_STROKE_MAX - EDGE_STROKE_MIN)
  )
}

/** backend の check_refs と同じ趣旨の参照検証。node.parent / edge.source / edge.target が nodes[].id に存在するか確認する */
export function validateGraphRefs(def: GraphDef): Result<void, Error> {
  const ids = new Set(def.nodes.map((n) => n.id))

  for (const node of def.nodes) {
    if (node.parent != null && !ids.has(node.parent)) {
      return err(
        new Error(
          `graph "${def.id}": node "${node.id}".parent = "${node.parent}" is not a known node id`,
        ),
      )
    }
  }
  for (const edge of def.edges) {
    if (!ids.has(edge.source)) {
      return err(
        new Error(
          `graph "${def.id}": edge.source = "${edge.source}" is not a known node id`,
        ),
      )
    }
    if (!ids.has(edge.target)) {
      return err(
        new Error(
          `graph "${def.id}": edge.target = "${edge.target}" is not a known node id`,
        ),
      )
    }
  }
  return ok(undefined)
}

/**
 * nodes[].cite → edges[].cite の順に出現順で走査し、初出の cite 文字列に 1 から連番を振る。
 * 同じ文字列は同じ番号になる。番号は描画時に振る (LLM に書かせると図の編集のたびに破綻するため)。
 */
export function computeCiteNumbers(def: GraphDef): Map<string, number> {
  const numbers = new Map<string, number>()
  let next = 1

  const visit = (cite: string | undefined) => {
    if (cite == null) return
    if (numbers.has(cite)) return
    numbers.set(cite, next)
    next += 1
  }

  for (const node of def.nodes) visit(node.cite)
  for (const edge of def.edges) visit(edge.cite)

  return numbers
}

export function buildFlowEdges(
  edges: GraphEdge[],
  citeNumbers: Map<string, number>,
): GraphFlowEdge[] {
  const maxValue = Math.max(0, ...edges.map((e) => e.value ?? 0))

  return edges.map((edge, index) => {
    const citeNumber =
      edge.cite != null ? citeNumbers.get(edge.cite) : undefined
    const label =
      citeNumber == null
        ? edge.label
        : edge.label != null
          ? `${edge.label} [${String(citeNumber)}]`
          : `[${String(citeNumber)}]`

    // ponytail: エッジの cite はテキスト注記のみにとどめる。ノードのような Popover 化が
    // 要るなら custom edge type + EdgeLabelRenderer で追加する
    return {
      id: `${edge.source}-${edge.target}-${String(index)}`,
      source: edge.source,
      target: edge.target,
      label,
      style: { strokeWidth: edgeStrokeWidth(edge.value, maxValue) },
      data: edge,
    }
  })
}
