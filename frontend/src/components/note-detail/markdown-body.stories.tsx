import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { http, HttpResponse } from 'msw'

import { CHANGE_REFERENCE_BARS } from '#components/note-detail/change-reference-bars.fixture'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { components } from '#lib/api/schema.gen'
import { mockResolveRef } from '#storybook/mock-resolve-ref'
import { createStoryRouter } from '#storybook/story-router'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:demo-code': 'Sample Stock',
  'indicator:demo-indicator': 'Sample Indicator',
  'group:demo-axis/demo-group': 'Sample Group',
}

const SAMPLE = `# サンプル銘柄の記録

## 要約

[[stock:demo-code]] はサンプル期間に 1,200-1,400 のレンジで推移している。[[indicator:demo-indicator]] と [[group:demo-axis/demo-group]] の動きも中立。こうした局面では **レンジ内での推移が続く可能性がある**。

## レンジ回帰の定量評価

過去の類似期間を抽出し、レンジ内で価格が推移した割合を計測した。

- 下限から中央へ戻った割合: **72%** (n=18)
- 上限を超えて推移した割合: 21%
- 中央へ戻るまでの営業日数: 4.3 日

直近の [[anno:A2]] で下限に接触し、終値は前日を上回った。

> 「確認ラインは 1,180」 — サンプル範囲の下限に設定

| ケース | 割合 | n |
| --- | ---: | ---: |
| 下限から中央へ戻る | 72% | 18 |
| 上限を超えて推移する | 21% | 18 |

詳細は [サンプル記事](https://example.com/sample-range) と [[group:demo-axis/demo-group]] の動向を参照。

判断の前提は [[note:00000000-0000-0000-0000-000000000101]] のバージョンに記録した。
現行の資料は [[note:00000000-0000-0000-0000-000000000102@current]] を参照する。

- 直近レンジ
    - 下限: 1,480
    - 上限: 1,640

\`\`\`python
print("nsjail で集計したサンプル")
\`\`\`
`

const NOTE_LINKS: components['schemas']['NoteLinkItem'][] = [
  {
    note_id: '00000000-0000-0000-0000-000000000101',
    version_id: '00000000-0000-0000-0000-000000000201',
    version_no: 2,
    title: 'サンプル銘柄の判断',
  },
  {
    note_id: '00000000-0000-0000-0000-000000000102',
    version_id: null,
    version_no: 4,
    title: 'サンプル環境の記録',
  },
]

const meta = {
  title: 'NoteDetail/MarkdownBody',
  component: MarkdownBody,
  parameters: {
    layout: 'padded',
    msw: {
      handlers: [
        mockResolveRef(NAMES),
        http.get('/api/bars', () => HttpResponse.json(CHANGE_REFERENCE_BARS)),
      ],
    },
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <QueryClientProvider client={queryClient}>
              <div className="max-w-3xl bg-background p-5 text-foreground">
                <Story />
              </div>
            </QueryClientProvider>
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof MarkdownBody>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'renders a formatted note with links to fixed and current versions.',
  args: { source: SAMPLE, noteLinks: NOTE_LINKS },
}

export const WithResolvedPriceReferences: Story = {
  name: 'renders resolved values and leaves a missing price token visible.',
  args: {
    source:
      '終値 [[price:fictional-code@2030-01-02:close]]、期間変化 [[change:fictional-code@2030-01-02..2030-01-03:close]]、未解決 [[price:fictional-code@2030-01-04:close]]',
    resolvedPriceReferences: {
      '[[price:fictional-code@2030-01-02:close]]': {
        value: 1234.5,
        evidence_id: '00000000-0000-0000-0000-000000000001',
      },
      '[[change:fictional-code@2030-01-02..2030-01-03:close]]': {
        value: 2.5,
        evidence_id: '00000000-0000-0000-0000-000000000002',
      },
    },
  },
}

// 以下のノード/ティッカーはすべて架空のもの。実在の企業・銘柄コードとは無関係
const GRAPH_SAMPLE = `# 架空エコシステムの業界構造メモ

前工程は ACME Litho [[stock:ACME]] と Nortek Materials [[stock:NRTK]] が押さえており、
受託製造の Fabrion Foundry [[stock:FBRN]] にほぼ集約される。

[[graph:g1]]

Fabrion の生産能力が QuantumX [[stock:QNTX]] の供給制約になっている点に注意。
`

const GRAPH_DEF: components['schemas']['GraphDef'] = {
  id: 'g1',
  layout: 'flow',
  title: '架空エコシステムの業界構造',
  nodes: [
    { id: 'acme', label: 'ACME Litho', ref: 'stock:ACME' },
    { id: 'nortek', label: 'Nortek Materials', ref: 'stock:NRTK' },
    { id: 'fabrion', label: 'Fabrion Foundry', ref: 'stock:FBRN' },
    { id: 'quantumx', label: 'QuantumX', ref: 'stock:QNTX' },
  ],
  edges: [
    { source: 'acme', target: 'fabrion', label: '露光装置' },
    { source: 'nortek', target: 'fabrion', label: '成膜材料' },
    { source: 'fabrion', target: 'quantumx', label: '受託生産' },
  ],
}

export const WithGraph: Story = {
  name: 'renders a markdown note alongside an embedded graph.',
  args: { source: GRAPH_SAMPLE, graphs: [GRAPH_DEF] },
}
