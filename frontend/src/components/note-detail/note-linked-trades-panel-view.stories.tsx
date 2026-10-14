import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteLinkedTradesPanelView } from '#components/note-detail/note-linked-trades-panel-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteLinkedTrade = components['schemas']['NoteLinkedTrade']

const linkedTrade: NoteLinkedTrade = {
  id: '00000000-0000-0000-0000-000000000101',
  date: '2026-08-12',
  symbol: 'demo-code',
  side: 'buy',
}

const meta = {
  title: 'NoteDetail/NoteLinkedTradesPanel',
  component: NoteLinkedTradesPanelView,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <div className="max-w-md bg-background p-5 text-foreground">
              <Story />
            </div>
          ),
          { paths: ['/trades/$tradeId', '/stocks/$stockId'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof NoteLinkedTradesPanelView>

export default meta
type Story = StoryObj<typeof NoteLinkedTradesPanelView>

export const Linked: Story = {
  name: 'shows a linked trade with its date, stock, and side.',
  args: {
    state: 'ready',
    trades: [linkedTrade],
    stockNameById: new Map([['demo-code', '架空商事']]),
  },
}

export const Loading: Story = {
  name: 'shows placeholders while linked trades are fetched.',
  args: { state: 'loading' },
}

export const Error: Story = {
  name: 'shows an error when linked trades cannot be loaded.',
  args: { state: 'error' },
}
