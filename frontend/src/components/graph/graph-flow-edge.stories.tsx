import '@xyflow/react/dist/style.css'

import type { Meta, StoryObj } from '@storybook/react-vite'
import { type EdgeTypes, type Node, ReactFlow } from '@xyflow/react'

import type { GraphFlowEdge } from '#components/graph/flow-types'
import { GraphFlowEdgeView } from '#components/graph/graph-flow-edge'

const edgeTypes: EdgeTypes = {
  graphEdge: GraphFlowEdgeView,
}

const nodes: Node[] = [
  {
    id: 'source',
    position: { x: 120, y: 50 },
    data: { label: 'Source node' },
    style: {
      width: 160,
      height: 56,
      backgroundColor: 'var(--card)',
      color: 'var(--card-foreground)',
      border: '1px solid var(--border)',
    },
  },
  {
    id: 'intermediate',
    position: { x: 340, y: 50 },
    data: { label: 'Intermediate node' },
    style: {
      width: 120,
      height: 56,
      backgroundColor: 'var(--card)',
      color: 'var(--card-foreground)',
      border: '1px solid var(--border)',
    },
  },
  {
    id: 'target',
    position: { x: 500, y: 50 },
    data: { label: 'Target node' },
    style: {
      width: 160,
      height: 56,
      backgroundColor: 'var(--card)',
      color: 'var(--card-foreground)',
      border: '1px solid var(--border)',
    },
  },
]

const edges: GraphFlowEdge[] = [
  {
    id: 'source-target',
    type: 'graphEdge',
    source: 'source',
    target: 'target',
    label: '架空ノード間の関係',
    data: {
      source: 'source',
      target: 'target',
      label: '架空ノード間の関係',
      dagre: {
        points: [
          { x: 280, y: 78 },
          { x: 310, y: 78 },
          { x: 310, y: 20 },
          { x: 470, y: 20 },
          { x: 470, y: 78 },
          { x: 500, y: 78 },
        ],
        labelPosition: { x: 390, y: 20 },
      },
    },
  },
]

function GraphFlowEdgeExample() {
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      edgeTypes={edgeTypes}
      nodesDraggable={false}
      nodesConnectable={false}
      defaultViewport={{ x: 0, y: 0, zoom: 1 }}
    />
  )
}

const meta = {
  title: 'Graph/GraphFlowEdge',
  component: GraphFlowEdgeExample,
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '180px' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof GraphFlowEdgeExample>

export default meta
type Story = StoryObj<typeof meta>

export const DagreRoute: Story = {
  name: 'routes around an intermediate node and places the label on its reserved segment.',
}
