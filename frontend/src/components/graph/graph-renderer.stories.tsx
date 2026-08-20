import type { Meta, StoryObj } from '@storybook/react-vite'

import { GraphRenderer } from '#components/graph/graph-renderer'
import type { GraphDef } from '#components/graph/types'

const meta = {
  title: 'Graph/GraphRenderer',
  component: GraphRenderer,
  // fitView のパン/ズームアニメーションを overflow-check が検査してしまわない
  // よう、story では即時反映にする (本番は既定の 200ms を維持)。
  // className: react-flow は `.react-flow { height: 100% }` で親の高さを継承する。
  // 未指定だと decorator の固定高さ div から高さが伝播せず 0 に潰れ、何も描画されない
  args: { fitViewDuration: 0, className: 'h-full' },
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '500px' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof GraphRenderer>

export default meta
type Story = StoryObj<typeof meta>

// 以下のノード/ティッカーはすべて架空のもの。実在の企業・銘柄コードとは無関係

const FLOW_DEF: GraphDef = {
  id: 'demo-flow',
  layout: 'flow',
  title: '架空エコシステムの業界構造',
  nodes: [
    { id: 'upstream', label: '前工程 (装置・材料)' },
    { id: 'acme', label: 'ACME Litho', ref: 'stock:ACME', parent: 'upstream' },
    {
      id: 'nortek',
      label: 'Nortek Materials',
      ref: 'stock:NRTK',
      parent: 'upstream',
    },
    { id: 'fab', label: '受託製造' },
    {
      id: 'fabrion',
      label: 'Fabrion Foundry',
      ref: 'stock:FBRN',
      value: 60,
      cite: '自社調査ノート: fab_market_share',
      parent: 'fab',
    },
    { id: 'quantumx', label: 'QuantumX', ref: 'stock:QNTX', value: 35 },
  ],
  edges: [
    { source: 'acme', target: 'fabrion', label: '露光装置', value: 51 },
    { source: 'nortek', target: 'fabrion', label: '成膜材料', value: 30 },
    {
      source: 'fabrion',
      target: 'quantumx',
      label: '受託生産',
      value: 45,
      cite: '自社調査ノート: fab_market_share',
    },
  ],
}

export const Flow: Story = {
  args: { def: FLOW_DEF },
}

const TREE_DEF: GraphDef = {
  id: 'demo-tree',
  layout: 'tree',
  title: '架空企業の売上ドライバーツリー',
  nodes: [
    { id: 'revenue', label: '総売上' },
    { id: 'domestic', label: '国内事業' },
    { id: 'overseas', label: '海外事業' },
    { id: 'hardware', label: 'ハードウェア' },
    { id: 'subscription', label: 'サブスクリプション' },
  ],
  edges: [
    { source: 'revenue', target: 'domestic', label: '寄与度', value: 40 },
    { source: 'revenue', target: 'overseas', label: '寄与度', value: 60 },
    { source: 'overseas', target: 'hardware', label: '寄与度', value: 25 },
    { source: 'overseas', target: 'subscription', label: '寄与度', value: 35 },
  ],
}

export const Tree: Story = {
  args: { def: TREE_DEF },
}

const CHAIN_DEF: GraphDef = {
  id: 'demo-chain',
  layout: 'chain',
  title: '架空バリューチェーン',
  nodes: [
    { id: 'sourcing', label: '原材料調達', value: 15 },
    { id: 'manufacturing', label: '製造', value: 35 },
    { id: 'distribution', label: '販売', value: 25 },
    { id: 'aftercare', label: 'アフターサービス', value: 60 },
  ],
  edges: [
    { source: 'sourcing', target: 'manufacturing' },
    { source: 'manufacturing', target: 'distribution' },
    { source: 'distribution', target: 'aftercare' },
  ],
}

export const Chain: Story = {
  args: { def: CHAIN_DEF },
}

const SCATTER_DEF: GraphDef = {
  id: 'demo-scatter',
  layout: 'scatter',
  title: '架空 2x2 競争優位マトリクス',
  nodes: [
    { id: 'a', label: 'Aster Corp', ref: 'stock:ASTR', x: 80, y: 85 },
    { id: 'b', label: 'Brightline', ref: 'stock:BRTL', x: 20, y: 70 },
    { id: 'c', label: 'Coral Systems', ref: 'stock:CRLS', x: 65, y: 20 },
    { id: 'd', label: 'Driftwood', ref: 'stock:DRFT', x: 15, y: 10 },
  ],
  edges: [],
}

export const Scatter: Story = {
  args: { def: SCATTER_DEF },
}
