import { Position } from '@xyflow/react'
import { err, ok, type Result } from 'neverthrow'

import type {
  GraphFlowEdge,
  GraphFlowNode,
  ScatterBackgroundFlowNode,
} from '#components/graph/flow-types'
import {
  buildFlowEdges,
  NODE_HEIGHT,
  nodeWidth,
  validateGraphRefs,
} from '#components/graph/graph-utils'
import type { GraphDef } from '#components/graph/types'

const CHAIN_GAP = 64

export function buildChainLayout(
  def: GraphDef,
  citeNumbers: Map<string, number>,
): Result<{ nodes: GraphFlowNode[]; edges: GraphFlowEdge[] }, Error> {
  const validation = validateGraphRefs(def)
  if (validation.isErr()) return err(validation.error)

  let x = 0
  const nodes: GraphFlowNode[] = def.nodes.map((node) => {
    const flowNode: GraphFlowNode = {
      id: node.id,
      type: 'graphNode',
      position: { x, y: 0 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: node,
    }
    x += nodeWidth(node) + CHAIN_GAP
    return flowNode
  })

  return ok({ nodes, edges: buildFlowEdges(def.edges, citeNumbers) })
}

export const SCATTER_BACKGROUND_ID = '__scatter_background__'

const SCATTER_SIZE = 560
const SCATTER_PADDING_RATIO = 0.2

function paddedDomain(values: number[]): [number, number] {
  const min = Math.min(...values)
  const max = Math.max(...values)
  const padding = Math.max((max - min) * SCATTER_PADDING_RATIO, 1)
  return [min - padding, max + padding]
}

function scale(
  value: number,
  min: number,
  max: number,
  size: number,
  invert = false,
): number {
  if (max <= min) return size / 2
  const t = (value - min) / (max - min)
  return (invert ? 1 - t : t) * size
}

export function buildScatterLayout(
  def: GraphDef,
  citeNumbers: Map<string, number>,
): Result<
  {
    nodes: (GraphFlowNode | ScatterBackgroundFlowNode)[]
    edges: GraphFlowEdge[]
  },
  Error
> {
  const validation = validateGraphRefs(def)
  if (validation.isErr()) return err(validation.error)

  const [xMin, xMax] = paddedDomain(def.nodes.map((n) => n.x ?? 0))
  const [yMin, yMax] = paddedDomain(def.nodes.map((n) => n.y ?? 0))

  const background: ScatterBackgroundFlowNode = {
    id: SCATTER_BACKGROUND_ID,
    type: 'graphScatterBackground',
    position: { x: 0, y: 0 },
    style: { width: SCATTER_SIZE, height: SCATTER_SIZE },
    zIndex: -1,
    selectable: false,
    draggable: false,
    connectable: false,
    data: {},
  }

  const dataNodes: GraphFlowNode[] = def.nodes.map((node) => {
    // 中心が象限座標に来るよう、ノード自身の半幅・半高を引く
    const cx = scale(node.x ?? 0, xMin, xMax, SCATTER_SIZE)
    const cy = scale(node.y ?? 0, yMin, yMax, SCATTER_SIZE, true)
    return {
      id: node.id,
      type: 'graphNode',
      position: { x: cx - nodeWidth(node) / 2, y: cy - NODE_HEIGHT / 2 },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: node,
    }
  })

  return ok({
    nodes: [background, ...dataNodes],
    edges: buildFlowEdges(def.edges, citeNumbers),
  })
}
