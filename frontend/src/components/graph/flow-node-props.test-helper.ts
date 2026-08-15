import type { NodeProps } from '@xyflow/react'

import type { GraphFlowNode } from '#components/graph/flow-types'
import type { GraphNode } from '#components/graph/types'

export function buildNodeProps(
  data: GraphNode,
  type: GraphFlowNode['type'],
): NodeProps<GraphFlowNode> {
  return {
    id: data.id,
    data,
    type,
    dragging: false,
    zIndex: 0,
    selectable: true,
    deletable: true,
    selected: false,
    draggable: true,
    isConnectable: true,
    positionAbsoluteX: 0,
    positionAbsoluteY: 0,
  }
}
