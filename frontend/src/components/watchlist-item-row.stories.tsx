import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { WatchlistItemRowView } from '#components/watchlist-item-row'
import { createStoryRouter } from '#storybook/story-router'

const meta = {
  title: 'Components/WatchlistItemRow',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemRowView
          item={{
            watchlist_id: '123',
            instrument_id: '7203',
            sort_order: 0,
            added_at: '2026-01-01T00:00:00Z',
          }}
          name="トヨタ自動車"
          isDeleting={false}
          onDelete={() => {}}
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const WithoutName: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemRowView
          item={{
            watchlist_id: '123',
            instrument_id: '9984',
            sort_order: 1,
            added_at: '2026-01-01T00:00:00Z',
          }}
          name={undefined}
          isDeleting={false}
          onDelete={() => {}}
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}

export const Deleting: Story = {
  render: () => {
    const router = createStoryRouter(
      () => (
        <WatchlistItemRowView
          item={{
            watchlist_id: '123',
            instrument_id: '7203',
            sort_order: 0,
            added_at: '2026-01-01T00:00:00Z',
          }}
          name="トヨタ自動車"
          isDeleting={true}
          onDelete={() => {}}
        />
      ),
      { paths: ['/charts/$instrumentId'] },
    )
    return <RouterProvider router={router} />
  },
}
