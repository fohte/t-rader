import type { Edge, Node } from '@xyflow/react'

import type { GraphEdge, GraphNode } from '#components/graph/types'

export type GraphFlowNode = Node<GraphNode, 'graphNode' | 'graphGroup'>
export type GraphFlowEdgeData = GraphEdge & {
  dagre?: {
    points: { x: number; y: number }[]
    labelPosition?: { x: number; y: number }
  }
}
export type GraphFlowEdge = Edge<GraphFlowEdgeData, 'graphEdge'> & {
  data: GraphFlowEdgeData
}
export type ScatterBackgroundFlowNode = Node<
  Record<string, unknown>,
  'graphScatterBackground'
>
