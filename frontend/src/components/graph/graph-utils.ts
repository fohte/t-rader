import { Position } from '@xyflow/react'
import { err, ok, type Result } from 'neverthrow'

import type { GraphFlowEdge } from '#components/graph/flow-types'
import type {
  GraphDef,
  GraphEdge,
  GraphNode,
  Layout,
} from '#components/graph/types'

const NODE_WIDTH_BASE = 160
export const NODE_HEIGHT = 56
// nodeHeight は graph-node.tsx の px-3 / py-2 / text-sm / gap-1.5 と RefChip の高さを基準にする
const NODE_HORIZONTAL_PADDING = 26
const NODE_VERTICAL_PADDING = 18
const NODE_LABEL_FONT_SIZE = 14
const NODE_LABEL_LINE_HEIGHT = 20
const NODE_CONTENT_GAP = 6
const REF_CHIP_HEIGHT = 20
const EDGE_LABEL_FONT_SIZE = 12
const EDGE_LABEL_HEIGHT = 20
const EDGE_LABEL_HORIZONTAL_PADDING = 12
const textSegmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' })

const EDGE_STROKE_MIN = 1.5
const EDGE_STROKE_MAX = 5

/** value を持つノードは幅を少し広げる (例: value 60 → 208px) */
export function nodeWidth(node: GraphNode): number {
  if (typeof node.value !== 'number') return NODE_WIDTH_BASE
  return NODE_WIDTH_BASE + Math.min(node.value, 100) * 0.8
}

function isWideCharacter(character: string): boolean {
  const codePoint = character.codePointAt(0) ?? 0
  return (
    (codePoint >= 0x2e80 && codePoint <= 0xa4cf) ||
    (codePoint >= 0xac00 && codePoint <= 0xd7af) ||
    (codePoint >= 0xf900 && codePoint <= 0xfaff) ||
    (codePoint >= 0xfe30 && codePoint <= 0xfe6f) ||
    (codePoint >= 0xff01 && codePoint <= 0xff60) ||
    (codePoint >= 0xffe0 && codePoint <= 0xffe6) ||
    (codePoint >= 0x1f000 && codePoint <= 0x1faff) ||
    (codePoint >= 0x20000 && codePoint <= 0x3ffff)
  )
}

function estimateTextWidth(text: string, fontSize: number): number {
  let width = 0
  for (const { segment: character } of textSegmenter.segment(text)) {
    if (/\s/u.test(character)) {
      width += fontSize * 0.3
      continue
    }
    width += fontSize * (isWideCharacter(character) ? 1 : 0.62)
  }
  return width
}

/** Dagre と描画側で同じ高さを使い、ラベルの折り返し後にノードが動かないようにする */
export function nodeHeight(node: GraphNode): number {
  const contentWidth = nodeWidth(node) - NODE_HORIZONTAL_PADDING
  const labelLines = Math.max(
    1,
    Math.ceil(
      estimateTextWidth(node.label, NODE_LABEL_FONT_SIZE) / contentWidth,
    ),
  )
  return (
    NODE_VERTICAL_PADDING +
    labelLines * NODE_LABEL_LINE_HEIGHT +
    (node.ref == null ? 0 : NODE_CONTENT_GAP + REF_CHIP_HEIGHT)
  )
}

export function estimateEdgeLabelSize(label: string): {
  width: number
  height: number
} {
  return {
    width:
      Math.ceil(estimateTextWidth(label, EDGE_LABEL_FONT_SIZE)) +
      EDGE_LABEL_HORIZONTAL_PADDING,
    height: EDGE_LABEL_HEIGHT,
  }
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

function edgeStrokeWidth(value: number | undefined, maxValue: number): number {
  if (typeof value !== 'number' || maxValue <= 0) return EDGE_STROKE_MIN
  return (
    EDGE_STROKE_MIN + (value / maxValue) * (EDGE_STROKE_MAX - EDGE_STROKE_MIN)
  )
}

/** node.parent / edge.source / edge.target が nodes[].id に存在するか確認する参照検証 */
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

    return {
      id: `${edge.source}-${edge.target}-${String(index)}`,
      type: 'graphEdge',
      source: edge.source,
      target: edge.target,
      label,
      style: { strokeWidth: edgeStrokeWidth(edge.value, maxValue) },
      data: edge,
    }
  })
}
