import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { TradeNotesDialogView } from '#components/trades/trade-notes-dialog-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']
type Trade = components['schemas']['TradeListItem']

const strategyId = '00000000-0000-0000-0000-000000000001'

const trade: Trade = {
  id: '00000000-0000-0000-0000-000000000101',
  strategy_id: strategyId,
  symbol: 'FICT1',
  side: 'buy',
  qty: 20,
  price: 1250,
  fee: 0,
  date: '2026-04-18',
  source: 'manual',
  note: null,
  note_count: 1,
  created_at: '2026-04-18T00:00:00Z',
  updated_at: '2026-04-18T00:00:00Z',
}

function noteStub(id: string, title: string): Note {
  return {
    id,
    version_id: '00000000-0000-0000-0000-000000000301',
    version_no: 1,
    is_current: true,
    strategy_id: strategyId,
    title,
    body_md: '検証用のメモです。',
    frontmatter_json: {},
    graphs_json: [],
    status: 'unread',
    created_by_kind: 'human',
    created_at: '2026-04-17T00:00:00Z',
    updated_at: '2026-04-17T00:00:00Z',
  }
}

const linkedNotes = [
  noteStub('00000000-0000-0000-0000-000000000201', '架空銘柄の購入判断'),
]
const candidateNotes = [
  noteStub('00000000-0000-0000-0000-000000000202', '検証用の観察メモ'),
  noteStub('00000000-0000-0000-0000-000000000203', '売買方針の記録'),
]

const meta = {
  title: 'Trades/TradeNotesDialogView',
  component: TradeNotesDialogView,
  parameters: { layout: 'centered' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          {
            paths: ['/notes/$noteId'],
          },
        )}
      />
    ),
  ],
  args: {
    open: true,
    onOpenChange: () => {},
    trade,
    linkedNotes,
    candidateNotes,
    search: '',
    onSearchChange: () => {},
    linkedNotesPending: false,
    candidateNotesPending: false,
    loadingError: false,
    operationError: null,
    operationPending: false,
    onLink: () => {},
    onUnlink: () => {},
  },
} satisfies Meta<typeof TradeNotesDialogView>

export default meta
type Story = StoryObj<typeof meta>

export const LinkedAndAvailable: Story = {
  name: 'shows linked notes and notes available to link with a trade.',
}

export const NoLinkedNotes: Story = {
  name: 'shows a trade with no linked notes and available candidates.',
  args: { linkedNotes: [] },
}

export const NoAvailableNotes: Story = {
  name: 'shows linked notes when no more notes are available to link.',
  args: { candidateNotes: [] },
}

export const Loading: Story = {
  name: 'shows loading placeholders while trade notes are fetched.',
  args: {
    linkedNotes: [],
    candidateNotes: [],
    linkedNotesPending: true,
    candidateNotesPending: true,
  },
}

export const OperationError: Story = {
  name: 'shows an error after a note-link operation fails.',
  args: { operationError: 'ノートの紐付けに失敗しました。' },
}
