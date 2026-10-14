import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
type Trade = components['schemas']['TradeListItem']

const annotations: Annotation[] = [
  {
    id: 'demo-annotation-1',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-04-02T06:00:00Z',
    timestamp_start: null,
    price: null,
    text: '出来高を伴って直近高値を上抜けたデモ用メモです。',
    linked_note_id: null,
    status: 'unread',
    created_by_kind: 'llm',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-04-02T06:00:00Z',
    updated_at: '2030-04-02T06:00:00Z',
  },
  {
    id: 'demo-annotation-2',
    target_kind: 'stock',
    target_symbol: 'demo-code',
    timestamp: '2030-03-28T06:00:00Z',
    timestamp_start: null,
    price: null,
    text: '移動平均線付近で反発したデモ用メモです。',
    linked_note_id: null,
    status: 'approved',
    created_by_kind: 'human',
    execution_step_id: null,
    execution_task_id: null,
    created_at: '2030-03-28T06:00:00Z',
    updated_at: '2030-03-28T06:00:00Z',
  },
]

const trades: Trade[] = [
  {
    id: 'demo-trade-1',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'buy',
    qty: 100,
    price: 3200,
    fee: 0,
    date: '2030-04-01',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2030-04-01T06:00:00Z',
    updated_at: '2030-04-01T06:00:00Z',
  },
  {
    id: 'demo-trade-2',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'sell',
    qty: 50,
    price: 3300,
    fee: 0,
    date: '2030-04-03',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2030-04-03T06:00:00Z',
    updated_at: '2030-04-03T06:00:00Z',
  },
]

const notes: Note[] = [
  {
    body_md: '架空コードへの言及を含むデモ用ノートです。',
    created_at: '2030-04-02T06:00:00Z',
    created_by_kind: 'llm',
    frontmatter_json: {},
    graphs_json: [],
    id: 'demo-note-newer',
    is_current: true,
    resolved_price_references_json: {},
    status: 'unread',
    tags: [],
    title: '銘柄を確認したデモ用ノート',
    updated_at: '2030-04-02T06:00:00Z',
    version_id: 'demo-note-version-newer',
    version_no: 1,
  },
  {
    body_md: '別の架空コードへの言及を含むデモ用ノートです。',
    created_at: '2030-03-28T06:00:00Z',
    created_by_kind: 'human',
    frontmatter_json: {},
    graphs_json: [],
    id: 'demo-note-older',
    is_current: true,
    resolved_price_references_json: {},
    status: 'approved',
    tags: [],
    title: '以前の確認メモ',
    updated_at: '2030-03-28T06:00:00Z',
    version_id: 'demo-note-version-older',
    version_no: 1,
  },
]

const meta = {
  title: 'StockPage/StockPageActivity',
  component: StockPageActivity,
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/annotations/$annoId', '/notes/$noteId'],
          },
        )}
      />
    ),
  ],
  args: {
    annotations,
    notes,
    trades,
    activeTab: 'annotations',
    onTabChange: () => undefined,
    selectedAnnotationId: 'demo-annotation-1',
    onSelectAnnotation: () => undefined,
  },
} satisfies Meta<typeof StockPageActivity>

export default meta
type Story = StoryObj<typeof meta>

export const Annotations: Story = {
  name: '未レビュー表示と選択状態のあるアノテーションを表示する。',
}

export const Trades: Story = {
  name: '取引を新しい順に表示する。',
  args: { activeTab: 'trades' },
}

export const Notes: Story = {
  name: 'ノートを新しい順に表示し、書いた人と未レビュー状態を示す。',
  args: { activeTab: 'notes' },
}

export const EmptyNotes: Story = {
  name: '該当するノートがない状態を表示する。',
  args: { activeTab: 'notes', notes: [] },
}

export const LoadingNotes: Story = {
  name: 'ノートを読み込み中の状態を表示する。',
  args: { activeTab: 'notes', noteState: 'loading' },
}

export const NoteError: Story = {
  name: 'ノートの取得に失敗した状態を表示する。',
  args: { activeTab: 'notes', noteState: 'error' },
}

export const Empty: Story = {
  name: 'アノテーションがない状態を表示する。',
  args: { annotations: [], selectedAnnotationId: null },
}

export const Loading: Story = {
  name: 'アノテーションの読み込み中を表示する。',
  args: { annotationState: 'loading' },
}

export const AnnotationError: Story = {
  name: 'アノテーションの取得エラーを表示する。',
  args: { annotationState: 'error' },
}

export const EmptyTrades: Story = {
  name: '取引がない状態を表示する。',
  args: { activeTab: 'trades', trades: [] },
}

export const LoadingTrades: Story = {
  name: '取引の読み込み中を表示する。',
  args: { activeTab: 'trades', tradeState: 'loading' },
}

export const TradeError: Story = {
  name: '取引の取得エラーを表示する。',
  args: { activeTab: 'trades', tradeState: 'error' },
}
