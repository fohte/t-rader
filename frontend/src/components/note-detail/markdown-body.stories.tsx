import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { components } from '#lib/api/schema.gen'

const SAMPLE = `# SUMCO レンジ回帰の確度評価

## 要約

SUMCO [[stock:3436]] は約 2 ヶ月にわたり 1,480-1,640 のレンジで推移している。[[indicator:USDJPY]] と [[sector:半導体]] のモメンタムも中立。こうした局面では **テクニカルなレンジ回帰が機能しやすい** という過去パターンに合致する。

## レンジ回帰の定量評価

過去 3 年の同様レジーム (材料なし・出来高低下・ボラ縮小) を抽出し、レンジ内回帰の発生率を計測した。

- レンジ滞在中に下限 -2σ から中央へ戻った確率: **72%** (n=18)
- 上限ブレイク継続 (ダマシでない) 確率: 21%
- 平均回帰までの営業日数: 4.3 日

直近の [[anno:A2]] で下限 -2σ に接触し下ヒゲ陽線。出来高も平均比 +38% と、反発シグナルの確度を補強する。

> 「撤退ラインは 1,470」 — レンジ下限 -2σ + ATR バッファ

| レジーム | 発生率 | n |
| --- | ---: | ---: |
| 下限 -2σ → 中央回帰 | 72% | 18 |
| 上限ブレイク継続 | 21% | 18 |

詳細は [参考記事](https://example.com/sumco-range) と [[sector:半導体]] の動向を参照。

- 直近レンジ
    - 下限: 1,480
    - 上限: 1,640

\`\`\`python
print("nsjail で集計したサンプル")
\`\`\`
`

const meta = {
  title: 'NoteDetail/MarkdownBody',
  component: MarkdownBody,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-[720px] bg-[color:var(--color-bg-primary)] p-5 text-[color:var(--color-text-primary)]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MarkdownBody>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: { source: SAMPLE },
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
  args: { source: GRAPH_SAMPLE, graphs: [GRAPH_DEF] },
}
