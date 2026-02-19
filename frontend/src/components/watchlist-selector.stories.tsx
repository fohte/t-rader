import type { Meta, StoryObj } from '@storybook/react-vite'

import { WatchlistSelectorView } from '@/components/watchlist-selector'

const meta = {
  title: 'Components/WatchlistSelector',
  component: WatchlistSelectorView,
} satisfies Meta<typeof WatchlistSelectorView>

export default meta
type Story = StoryObj<typeof meta>

const sampleWatchlists = [
  {
    id: '1',
    name: 'メインウォッチリスト',
    sort_order: 0,
    created_at: '2026-01-01T00:00:00Z',
  },
  {
    id: '2',
    name: '高配当銘柄',
    sort_order: 1,
    created_at: '2026-01-02T00:00:00Z',
  },
  {
    id: '3',
    name: 'テック銘柄',
    sort_order: 2,
    created_at: '2026-01-03T00:00:00Z',
  },
]

export const Default: Story = {
  args: {
    watchlists: sampleWatchlists,
    selectedId: '1',
    onSelect: () => {},
    isCreating: false,
    isDeleting: false,
    onCreateSubmit: () => {},
    onDelete: () => {},
  },
}

export const NoSelection: Story = {
  args: {
    watchlists: sampleWatchlists,
    selectedId: null,
    onSelect: () => {},
    isCreating: false,
    isDeleting: false,
    onCreateSubmit: () => {},
    onDelete: () => {},
  },
}

export const SingleWatchlist: Story = {
  args: {
    watchlists: sampleWatchlists.slice(0, 1),
    selectedId: '1',
    onSelect: () => {},
    isCreating: false,
    isDeleting: false,
    onCreateSubmit: () => {},
    onDelete: () => {},
  },
}

export const Empty: Story = {
  args: {
    watchlists: [],
    selectedId: null,
    onSelect: () => {},
    isCreating: false,
    isDeleting: false,
    onCreateSubmit: () => {},
    onDelete: () => {},
  },
}
