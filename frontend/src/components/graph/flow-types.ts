import type { Edge, Node } from '@xyflow/react'

import type { GraphEdge, GraphNode } from '#components/graph/types'

export type GraphFlowNode = Node<GraphNode, 'graphNode' | 'graphGroup'>
export type GraphFlowEdge = Edge<GraphEdge, 'graphEdge'>
export type ScatterBackgroundFlowNode = Node<
  Record<string, unknown>,
  'graphScatterBackground'
>
