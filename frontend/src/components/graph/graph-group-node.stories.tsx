import type { Meta, StoryObj } from '@storybook/react-vite'
import type { NodeProps } from '@xyflow/react'

import type { GraphFlowNode } from '#components/graph/flow-types'
import { GraphGroupNodeView } from '#components/graph/graph-group-node'
import { GraphRenderContextProvider } from '#components/graph/graph-render-context'
import type { GraphNode } from '#components/graph/types'

const NODE_ID = 'demo-group'
const NODE_DATA: GraphNode = { id: NODE_ID, label: 'デモグループ' }

function buildNodeProps(): NodeProps<GraphFlowNode> {
  return {
    id: NODE_ID,
    type: 'graphGroup',
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

function GraphGroupNodePreview({ dimmed }: { dimmed: boolean }) {
  return (
    <GraphRenderContextProvider
      value={{
        layout: 'flow',
        maxNodeValue: 0,
        citeNumbers: new Map(),
        highlightedNodeIds: dimmed ? new Set() : new Set([NODE_ID]),
      }}
    >
      <div className="h-25 w-60">
        <GraphGroupNodeView {...buildNodeProps()} />
      </div>
    </GraphRenderContextProvider>
  )
}

const meta = {
  title: 'Graph/GraphGroupNodeView',
  component: GraphGroupNodePreview,
  args: { dimmed: false },
} satisfies Meta<typeof GraphGroupNodePreview>

export default meta
type Story = StoryObj<typeof meta>

export const Highlighted: Story = {
  name: 'The group stays highlighted while it is under the pointer.',
  args: { dimmed: false },
}

export const Dimmed: Story = {
  name: 'The group is dimmed when it is outside the hovered neighborhood.',
  args: { dimmed: true },
}
