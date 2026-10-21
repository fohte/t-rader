import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'

import { RssFeedsPageView } from '#components/settings/rss-feeds-page-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type RssFeed = components['schemas']['RssFeed']

const queryClient = new QueryClient()
const exampleFeeds: RssFeed[] = [
  {
    id: '00000000-0000-0000-0000-000000000002',
    source: 'sample-feed-a',
    display_name: 'Example Feed A',
    url: 'https://feeds.example.invalid/a.xml',
    enabled: true,
    content_source: 'none',
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-000000000003',
    source: 'sample-feed-b',
    display_name: 'Example Feed B',
    url: 'https://feeds.example.invalid/b.xml',
    enabled: false,
    content_source: 'none',
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-01T00:00:00Z',
  },
]

const onFeedAction = () => {}
const onDialogOpenChange = () => {}

const meta = {
  title: 'Settings/RssFeedsPageView',
  component: RssFeedsPageView,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <RouterProvider
          router={createStoryRouter(
            () => (
              <Story />
            ),
            { paths: ['/settings'] },
          )}
        />
      </QueryClientProvider>
    ),
  ],
} satisfies Meta<typeof RssFeedsPageView>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows the loading placeholder while feeds are being fetched.',
  args: {
    feeds: undefined,
    isPending: true,
    dialogOpen: false,
    editingFeed: null,
    onAddFeed: onFeedAction,
    onToggleEnabled: onFeedAction,
    onEditFeed: onFeedAction,
    onDeleteFeed: onFeedAction,
    onDialogOpenChange,
  },
}

export const Empty: Story = {
  name: 'shows an empty state when no feeds have been registered.',
  args: {
    feeds: [],
    isPending: false,
    dialogOpen: false,
    editingFeed: null,
    onAddFeed: onFeedAction,
    onToggleEnabled: onFeedAction,
    onEditFeed: onFeedAction,
    onDeleteFeed: onFeedAction,
    onDialogOpenChange,
  },
}

export const Feeds: Story = {
  name: 'shows enabled and disabled feeds with their available actions.',
  args: {
    feeds: exampleFeeds,
    isPending: false,
    dialogOpen: false,
    editingFeed: null,
    onAddFeed: onFeedAction,
    onToggleEnabled: onFeedAction,
    onEditFeed: onFeedAction,
    onDeleteFeed: onFeedAction,
    onDialogOpenChange,
  },
}
