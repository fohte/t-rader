import '@xyflow/react/dist/style.css'

import type { Meta, StoryObj } from '@storybook/react-vite'
import { type NodeProps, ReactFlowProvider } from '@xyflow/react'

import type { GraphFlowNode } from '#components/graph/flow-types'
import { GraphNodeView } from '#components/graph/graph-node'
import { GraphRenderContextProvider } from '#components/graph/graph-render-context'
import type { GraphNode } from '#components/graph/types'

const NODE_ID = 'demo-node'
const NODE_DATA: GraphNode = { id: NODE_ID, label: 'デモノード' }

function buildNodeProps(): NodeProps<GraphFlowNode> {
  return {
    id: NODE_ID,
    type: 'graphNode',
    data: NODE_DATA,
    dragging: false,
    zIndex: 0,
    selectable: true,
    deletable: true,
    selected: false,
    draggable: false,
    isConnectable: false,
    positionAbsoluteX: 0,
    positionAbsoluteY: 0,
  }
}

function GraphNodePreview({ dimmed }: { dimmed: boolean }) {
  return (
    <ReactFlowProvider>
      <GraphRenderContextProvider
        value={{
          layout: 'flow',
          maxNodeValue: 0,
          citeNumbers: new Map(),
          highlightedNodeIds: dimmed ? new Set() : new Set([NODE_ID]),
        }}
      >
        <GraphNodeView {...buildNodeProps()} />
      </GraphRenderContextProvider>
    </ReactFlowProvider>
  )
}

const meta = {
  title: 'Graph/GraphNodeView',
  component: GraphNodePreview,
  args: { dimmed: false },
} satisfies Meta<typeof GraphNodePreview>

export default meta
type Story = StoryObj<typeof meta>

export const Highlighted: Story = {
  name: 'The node stays highlighted while it is under the pointer.',
  args: { dimmed: false },
}

export const Dimmed: Story = {
  name: 'The node is dimmed when it is outside the hovered neighborhood.',
  args: { dimmed: true },
}
