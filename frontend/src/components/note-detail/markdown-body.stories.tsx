import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownBody } from '#components/note-detail/markdown-body'

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
