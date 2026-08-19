import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { WatchlistItemListView } from '#components/watchlist-item-list'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Components/WatchlistItemList',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

const sampleItems = [
  {
    watchlist_id: '123',
    instrument_id: '7203',
    sort_order: 0,
    added_at: '2026-01-01T00:00:00Z',
  },
  {
    watchlist_id: '123',
    instrument_id: '9984',
    sort_order: 1,
    added_at: '2026-01-02T00:00:00Z',
  },
  {
    watchlist_id: '123',
    instrument_id: '6758',
    sort_order: 2,
    added_at: '2026-01-03T00:00:00Z',
  },
]

const sampleNames = new Map([
  ['7203', 'トヨタ自動車'],
  ['9984', 'ソフトバンクグループ'],
  ['6758', 'ソニーグループ'],
])

export const WithItems: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemListView
          items={sampleItems}
          instrumentNames={sampleNames}
          watchlistId="123"
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const Empty: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemListView
          items={[]}
          instrumentNames={new Map()}
          watchlistId="123"
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const WithoutNames: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemListView
          items={sampleItems}
          instrumentNames={new Map()}
          watchlistId="123"
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}
