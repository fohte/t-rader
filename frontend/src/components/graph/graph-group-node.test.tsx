import { cleanup, render, screen } from '@testing-library/react'
import type { NodeProps } from '@xyflow/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { GraphFlowNode } from '#components/graph/flow-types'
import { GraphGroupNodeView } from '#components/graph/graph-group-node'
import type { GraphNode } from '#components/graph/types'

afterEach(cleanup)

function buildNodeProps(data: GraphNode): NodeProps<GraphFlowNode> {
  return {
    id: data.id,
    data,
    type: 'graphGroup',
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

describe('GraphGroupNodeView', () => {
  it('data.label を表示する', () => {
    render(
      <GraphGroupNodeView
        {...buildNodeProps({ id: 'group1', label: 'グループ1' })}
      />,
    )
    expect(screen.getByText('グループ1')).toBeInTheDocument()
  })
})
