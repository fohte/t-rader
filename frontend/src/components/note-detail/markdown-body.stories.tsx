import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { http, HttpResponse } from 'msw'

import { CHANGE_REFERENCE_BARS } from '#components/note-detail/change-reference-bars.fixtures'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import {
  NOTE_BODY_GUTTER_CARD_BLEED_REM,
  NOTE_BODY_GUTTER_CARD_PADDING_REM,
} from '#components/note-detail/markdown-body-range-comment-context'
import { getDiffCommentAnchorKey } from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'
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

1. 最初の手順
    - 子項目
2. 次の手順

\`\`\`python
print("nsjail で集計したサンプル")
\`\`\`
`

const GUTTER_LAYOUT_SAMPLE = `${'\n'.repeat(99)}${[
  '# 行番号付きの本文',
  '',
  '## 見出しと段落',
  '',
  '本文の位置と字下げを保つ。',
  '',
  '- 箇条書き',
  '    - 入れ子の項目',
  '',
  '1. 番号付きリスト',
  '    1. 入れ子の番号付きリスト',
  '',
  '> 引用文',
  '',
  '    コード行',
  '',
  '| 項目 | 状態 |',
  '| --- | --- |',
  '| 表の行 | 対象 |',
].join('\n')}`

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

const rangeComment: NoteVersionComment = {
  id: 'sample-range-comment',
  target_kind: 'note_version',
  target_id: 'sample-version',
  parent_id: null,
  body: 'この段落の根拠を補足してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '段落の内容です。',
  anchor_side: 'new',
  start_line: 3,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const formattedNoteComments: NoteVersionComment[] = [
  {
    id: 'nested-list-comment',
    target_kind: 'note_version',
    target_id: 'sample-version',
    parent_id: null,
    body: '入れ子の項目を確認した。',
    author_kind: 'human',
    author_label: 'レビュー担当者',
    resolved: false,
    anchor_text: '入れ子の項目',
    anchor_side: 'new',
    start_line: 107,
    end_line: 107,
    created_at: '2026-01-12T03:00:00Z',
  },
  {
    id: 'table-row-comment',
    target_kind: 'note_version',
    target_id: 'sample-version',
    parent_id: null,
    body: '表の行を確認した。',
    author_kind: 'human',
    author_label: 'レビュー担当者',
    resolved: false,
    anchor_text: '| 表の行 | 対象 |',
    anchor_side: 'new',
    start_line: 118,
    end_line: 118,
    created_at: '2026-01-12T03:00:00Z',
  },
]

const rangeCommentProps = {
  comments: [rangeComment],
  activeCommentAnchor: null,
  activeCommentKey: null,
  replyingCommentId: null,
  isCreating: false,
  isReplying: false,
  resolvingCommentId: null,
  hasCreateError: false,
  hasReplyError: false,
  hasResolveError: false,
  isCommentsPending: false,
  hasCommentsError: false,
  onStartComment: () => {},
  onCancelComment: () => {},
  onCreateComment: () => {},
  onStartReply: () => {},
  onCancelReply: () => {},
  onReply: () => {},
  onToggleResolved: () => {},
}

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

export const WithRangeCommentGutter: Story = {
  name: 'the formatted note keeps its body position beside the comment gutter.',
  decorators: [
    (Story) => (
      <div
        className="border border-border bg-card p-5 text-foreground"
        style={{
          marginLeft: `-${String(NOTE_BODY_GUTTER_CARD_BLEED_REM)}rem`,
          paddingLeft: `${String(NOTE_BODY_GUTTER_CARD_PADDING_REM)}rem`,
        }}
      >
        <Story />
      </div>
    ),
  ],
  args: {
    source: GUTTER_LAYOUT_SAMPLE,
    noteLinks: NOTE_LINKS,
    rangeComments: {
      ...rangeCommentProps,
      comments: formattedNoteComments,
      activeCommentAnchor: {
        side: 'new',
        lineNumber: 118,
        endLineNumber: 118,
        text: '| 表の行 | 対象 |',
      },
      activeCommentKey: getDiffCommentAnchorKey({
        side: 'new',
        lineNumber: 118,
        endLineNumber: 118,
        text: '',
      }),
    },
  },
}

export const WithResolvedPriceReferences: Story = {
  name: 'renders resolved price, valuation, indicator, and evaluation values.',
  args: {
    source:
      '終値 [[price:fictional-code@2030-01-02:close]]、期間変化 [[change:fictional-code@2030-01-02..2030-01-03:close]]、PER [[valuation:fictional-code@2030-01-02:per]]、為替 [[indicator:demo-indicator@2030-01-02]]、評価結果 [[eval:00000000-0000-0000-0000-000000000101:result]]、指標参照 [[indicator:demo-indicator]]、未解決 [[price:fictional-code@2030-01-04:close]]',
    resolvedPriceReferences: {
      '[[price:fictional-code@2030-01-02:close]]': {
        value: 1234.5,
      },
      '[[change:fictional-code@2030-01-02..2030-01-03:close]]': {
        value: 2.5,
      },
      '[[valuation:fictional-code@2030-01-02:per]]': { value: 24.6 },
      '[[indicator:demo-indicator@2030-01-02]]': { value: 152.3 },
      '[[eval:00000000-0000-0000-0000-000000000101:result]]': {
        value: 253.7,
      },
    },
  },
}

export const WithUnresolvedDatedReferences: Story = {
  name: 'keeps dated value tokens visible when their values are unavailable.',
  args: {
    source:
      '未解決の valuation [[valuation:demo-code@2030-01-04:per]] と indicator [[indicator:demo-indicator@2030-01-04]] は保存値がありません。日付なしの [[indicator:demo-indicator]] は参照リンクです。',
  },
}

export const WithRangeCommentThread: Story = {
  name: 'shows a comment thread below the final block in its range.',
  args: {
    source: '# 範囲コメント\n\n段落の内容です。\n次の行に続きます。',
    rangeComments: rangeCommentProps,
  },
}

export const WithCommentComposer: Story = {
  name: 'shows a comment composer below the selected source block.',
  args: {
    source: '# 範囲コメント\n\n段落の内容です。\n次の行に続きます。',
    rangeComments: {
      ...rangeCommentProps,
      activeCommentAnchor: {
        side: 'new',
        lineNumber: 3,
        endLineNumber: 4,
        text: '段落の内容です。\n次の行に続きます。',
      },
      activeCommentKey: getDiffCommentAnchorKey({
        side: 'new',
        lineNumber: 3,
        endLineNumber: 4,
        text: '',
      }),
      comments: [],
    },
  },
}

export const CommentsLoading: Story = {
  name: 'shows a loading message while note comments are fetched.',
  args: {
    source: '# コメントを読み込み中の本文\n\nコメント付きの段落です。',
    rangeComments: {
      ...rangeCommentProps,
      comments: [],
      isCommentsPending: true,
    },
  },
}

export const CommentsLoadFailed: Story = {
  name: 'shows an error when note comments cannot be loaded.',
  args: {
    source: '# コメントを読み込めない本文\n\nコメント付きの段落です。',
    rangeComments: {
      ...rangeCommentProps,
      comments: [],
      hasCommentsError: true,
    },
  },
}

export const ResolveFailed: Story = {
  name: 'shows an error when a comment cannot be resolved.',
  args: {
    source: '# 解決に失敗した本文\n\nコメント付きの段落です。',
    rangeComments: {
      ...rangeCommentProps,
      hasResolveError: true,
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

export const WithInteractiveReferences: Story = {
  name: 'makes stock, indicator, group, and annotation chips clickable.',
  args: {
    source:
      '[[stock:demo-code]] [[indicator:demo-indicator]] [[group:demo-axis/demo-group]] [[anno:00000000-0000-0000-0000-000000000301]]\n\n[[graph:g1]]',
    graphs: [GRAPH_DEF],
    onRef: () => undefined,
    onAnno: () => undefined,
  },
}
