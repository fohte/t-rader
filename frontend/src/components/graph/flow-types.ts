import type { Edge, Node } from '@xyflow/react'
import type { CSSProperties } from 'react'

import type { GraphEdge, GraphNode } from '#components/graph/types'

export type GraphNodeStyle = CSSProperties & {
  [property: `--graph-${string}`]: string
}

export type GraphFlowNode = Omit<
  Node<GraphNode, 'graphNode' | 'graphGroup'>,
  'style'
> & {
  style?: GraphNodeStyle
}
type GraphFlowEdgeData = GraphEdge & {
  dagre?: {
    points: { x: number; y: number }[]
    labelPosition?: { x: number; y: number }
  }
}
export type GraphFlowEdge = Edge<GraphFlowEdgeData, 'graphEdge'> & {
  data: GraphFlowEdgeData
}
export type ScatterBackgroundFlowNode = Omit<
  Node<Record<string, unknown>, 'graphScatterBackground'>,
  'style'
> & {
  style?: GraphNodeStyle
}
