import type { Decorator, Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { http, HttpResponse } from 'msw'

import { CHANGE_REFERENCE_BARS } from '#components/note-detail/change-reference-bars.fixtures'
import { MarkdownBody } from '#components/note-detail/markdown-body'
import { NoteBodyCard } from '#components/note-detail/note-body-card'
import { getDiffCommentAnchorKey } from '#components/note-detail/note-version-line-comment-group'
import type { NoteVersionComment } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import { mockResolveRef } from '#storybook/mock-resolve-ref'
import { createStoryRouter } from '#storybook/story-router'

const queryClient = new QueryClient()

const NAMES: Record<string, string> = {
  'stock:item-a': 'Item A',
  'indicator:metric-a': 'Measure A',
  'group:set/a': 'Set A',
  'group:set/b': 'Input B',
  'group:set/c': 'Input C',
  'group:set/d': 'Record',
  'group:set/e': 'Index',
}

const SAMPLE = `# サンプル項目の記録

## 要約

[[stock:item-a]] は記録期間に 12-18 件の範囲で確認された。[[indicator:metric-a]] と [[group:set/a]] の値も安定している。現在の記録から **同じ形式で項目を整理できる**。

## 項目の集計結果

過去の記録を抽出し、指定された形式に沿って整理できた割合を集計した。

- 形式に一致した割合: **72%** (n=18)
- 要確認の項目: 21%
- 整理にかかった時間: 4.3 分

直近の [[anno:mark-7]] で項目名の表記ゆれを確認した。

> 「一覧の 12 番から 18 番を再確認」 — 対象範囲を記録

| 区分 | 割合 | 件数 |
| --- | ---: | ---: |
| 形式に一致 | 72% | 18 |
| 要確認 | 21% | 18 |

詳細は [サンプル手順](https://example.com/sample-process) と [[group:set/a]] の記録を参照。

確認の前提は [[note:11111111-2222-4333-8444-555555555501]] のバージョンに記録した。
現行の資料は [[note:11111111-2222-4333-8444-555555555502@current]] を参照する。

- 対象の範囲
    - 開始番号: 12
    - 終了番号: 18

1. 項目を読み込む
    - 表記を整える
2. 一覧を保存する

\`\`\`python
print("サンプル項目を集計")
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
    note_id: '11111111-2222-4333-8444-555555555501',
    version_id: '11111111-2222-4333-8444-555555555601',
    version_no: 2,
    title: 'サンプル項目の確認',
  },
  {
    note_id: '11111111-2222-4333-8444-555555555502',
    version_id: null,
    version_no: 4,
    title: 'サンプル一覧の記録',
  },
]

const rangeComment: NoteVersionComment = {
  id: 'sample-item-comment',
  target_kind: 'note_version',
  target_id: 'synthetic-version',
  parent_id: null,
  body: 'この段落の説明を補足してください。',
  author_kind: 'human',
  author_label: 'レビュー担当者',
  resolved: false,
  anchor_text: '内容です。',
  anchor_side: 'new',
  start_line: 3,
  end_line: 4,
  created_at: '2026-01-12T03:00:00Z',
}

const formattedNoteComments: NoteVersionComment[] = [
  {
    id: 'nested-list-comment',
    target_kind: 'note_version',
    target_id: 'synthetic-version',
    parent_id: null,
    body: '入れ子の項目を記録した。',
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
    target_id: 'synthetic-version',
    parent_id: null,
    body: '表の行を記録した。',
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

const withNoteBodyCard: Decorator = (Story) => (
  <NoteBodyCard>
    <Story />
  </NoteBodyCard>
)

export const Default: Story = {
  name: 'renders a formatted note with links to fixed and current versions.',
  args: { source: SAMPLE, noteLinks: NOTE_LINKS },
}

export const WithRangeCommentGutter: Story = {
  name: 'the formatted note keeps its body position beside the comment gutter.',
  decorators: [withNoteBodyCard],
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
  name: 'renders resolved dated values and keeps unresolved values visible.',
  args: {
    source:
      '記録値 [[price:item-a@2030-01-02:close]]、期間差 [[change:item-a@2030-01-02..2030-01-03:close]]、比率 [[valuation:item-a@2030-01-02:per]]、補助値 [[indicator:metric-a@2030-01-02]]、算出結果 [[eval:11111111-2222-4333-8444-555555555501:result]]、項目参照 [[indicator:metric-a]]、未登録値 [[price:item-a@2030-01-04:close]]',
    resolvedPriceReferences: {
      '[[price:item-a@2030-01-02:close]]': {
        value: 1234.5,
      },
      '[[change:item-a@2030-01-02..2030-01-03:close]]': {
        value: 2.5,
      },
      '[[valuation:item-a@2030-01-02:per]]': { value: 24.6 },
      '[[indicator:metric-a@2030-01-02]]': { value: 152.3 },
      '[[eval:11111111-2222-4333-8444-555555555501:result]]': {
        value: 253.7,
      },
    },
  },
}

export const WithUnresolvedDatedReferences: Story = {
  name: 'keeps dated value tokens visible when their values are unavailable.',
  args: {
    source:
      '日付つきの値 [[valuation:item-a@2030-01-04:per]] と [[indicator:metric-a@2030-01-04]] は未登録です。日付なしの [[indicator:metric-a]] は項目参照です。',
  },
}

export const WithRangeCommentThread: Story = {
  name: 'shows a saved comment with its matching text highlighted.',
  decorators: [withNoteBodyCard],
  args: {
    source: '# 範囲コメント\n\n段落の内容です。\n次の行に続きます。',
    rangeComments: rangeCommentProps,
  },
}

export const WithCommentComposer: Story = {
  name: 'shows a comment composer below the selected source block.',
  decorators: [withNoteBodyCard],
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

export const WithTextSelectionCommentComposer: Story = {
  name: 'highlights the selected text while composing a body comment.',
  decorators: [withNoteBodyCard],
  args: {
    source: '# 範囲コメント\n\n段落の内容です。\n次の行に続きます。',
    rangeComments: {
      ...rangeCommentProps,
      comments: [],
      activeCommentAnchor: {
        side: 'new',
        lineNumber: 3,
        endLineNumber: 4,
        text: '内容です。',
        isTextSelection: true,
        textOffset: 3,
      },
      activeCommentKey: getDiffCommentAnchorKey({
        side: 'new',
        lineNumber: 3,
        endLineNumber: 4,
        text: '',
      }),
    },
  },
}

export const CommentsLoading: Story = {
  name: 'shows a loading message while note comments are fetched.',
  decorators: [withNoteBodyCard],
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
  decorators: [withNoteBodyCard],
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
  decorators: [withNoteBodyCard],
  args: {
    source: '# 解決に失敗した本文\n\nコメント付きの段落です。',
    rangeComments: {
      ...rangeCommentProps,
      hasResolveError: true,
    },
  },
}

const GRAPH_SAMPLE = `# サンプル資料の構成メモ

記録は [[group:set/b]] と [[group:set/c]] の項目から組み立て、
内容を [[group:set/d]] にまとめる。

[[graph:map-7]]

完成した項目は [[group:set/e]] に並べる。
`

const GRAPH_DEF: components['schemas']['GraphDef'] = {
  id: 'map-7',
  layout: 'flow',
  title: 'サンプル資料の構成',
  nodes: [
    { id: 'input-a', label: '入力 A', ref: 'group:set/b' },
    { id: 'input-b', label: '入力 B', ref: 'group:set/c' },
    { id: 'record', label: '記録票', ref: 'group:set/d' },
    { id: 'overview', label: '一覧', ref: 'group:set/e' },
  ],
  edges: [
    { source: 'input-a', target: 'record', label: '形式を適用' },
    { source: 'input-b', target: 'record', label: '内容を整理' },
    { source: 'record', target: 'overview', label: '結果を出力' },
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
      '[[stock:item-a]] [[indicator:metric-a]] [[group:set/a]] [[anno:mark-7]]\n\n[[graph:map-7]]',
    graphs: [GRAPH_DEF],
    onRef: () => undefined,
    onAnno: () => undefined,
  },
}
