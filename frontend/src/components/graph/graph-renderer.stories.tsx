import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { GraphRenderer } from '#components/graph/graph-renderer'
import type { GraphDef } from '#components/graph/types'
import { mockResolveRef } from '#storybook/mock-resolve-ref'

const queryClient = new QueryClient()

const meta = {
  title: 'Graph/GraphRenderer',
  component: GraphRenderer,
  // fitView のパン/ズームアニメーションを overflow-check が検査してしまわない
  // よう、story では即時反映にする (本番は既定の 200ms を維持)。
  // className: react-flow は `.react-flow { height: 100% }` で親の高さを継承する。
  // 未指定だと decorator の固定高さ div から高さが伝播せず 0 に潰れ、何も描画されない
  args: { fitViewDuration: 0, className: 'h-full' },
  parameters: {
    msw: { handlers: [mockResolveRef({})] },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="h-125 w-full">
          <Story />
        </div>
      </QueryClientProvider>
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
  name: 'shows an industry flow from suppliers to downstream companies.',
  args: { def: FLOW_DEF },
}

const FLOW_LABELS_DEF: GraphDef = {
  id: 'demo-flow-label-spacing',
  layout: 'flow',
  title: '架空の値動き観察',
  nodes: [
    { id: 'a1', label: '9/10 高値 12,345', cite: '架空の根拠' },
    {
      id: 'a2',
      label: 'AAAA 9/18 終値 11,111',
      ref: 'stock:AAAA',
      cite: '架空の根拠',
    },
    { id: 'a3', label: 'ギャップ帯下限 10,000', cite: '架空の根拠' },
    { id: 'b1', label: '7/30 安値 2,222', cite: '架空の根拠' },
    { id: 'b2', label: '供給ゾーン下端 3,000', cite: '架空の根拠' },
    {
      id: 'b3',
      label: 'BBBB 9/18 終値 2,900',
      ref: 'stock:BBBB',
      cite: '架空の根拠',
    },
    { id: 'c1', label: '安値 3 日連続切り上げ', cite: '架空の根拠' },
    {
      id: 'c2',
      label: 'CCCC 9/18 終値 1,500',
      ref: 'stock:CCCC',
      cite: '架空の根拠',
    },
    { id: 'c3', label: '9/15 安値 1,400', cite: '架空の根拠' },
  ],
  edges: [
    {
      source: 'a1',
      target: 'a3',
      label: '日続けて押し戻し済み',
      cite: '架空の根拠',
    },
    {
      source: 'a2',
      target: 'a3',
      label: '+1.13% で接近',
      cite: '架空の根拠',
    },
    {
      source: 'b1',
      target: 'b3',
      label: '下値確認後に接近',
      cite: '架空の根拠',
    },
    {
      source: 'b2',
      target: 'b3',
      label: '下抜未突破',
      cite: '架空の根拠',
    },
    {
      source: 'c1',
      target: 'c2',
      label: '安値切り上げ',
      cite: '架空の根拠',
    },
    {
      source: 'c2',
      target: 'c3',
      label: '未試験',
      cite: '架空の根拠',
    },
  ],
}

export const FlowWithLongLabels: Story = {
  name: 'keeps long labels and reference chips clear of nearby nodes.',
  args: { def: FLOW_LABELS_DEF },
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
  name: 'shows how revenue breaks down into business drivers.',
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
  name: 'shows a value chain from sourcing through aftercare.',
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
  name: 'maps companies by their competitive position.',
  args: { def: SCATTER_DEF },
}
