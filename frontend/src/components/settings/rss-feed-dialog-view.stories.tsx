import type { Meta, StoryObj } from '@storybook/react-vite'

import type { RssFeedDialogViewProps } from '#components/settings/rss-feed-dialog-view'
import { RssFeedDialogView } from '#components/settings/rss-feed-dialog-view'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

const feed: RssFeed = {
  id: '00000000-0000-0000-0000-000000000001',
  source: 'sample-feed',
  display_name: 'Example Feed',
  url: 'https://feeds.example.invalid/sample.xml',
  enabled: true,
  content_source: 'none',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}

const baseArgs: RssFeedDialogViewProps = {
  open: true,
  onOpenChange: () => {},
  feed: null,
  source: '',
  onSourceChange: () => {},
  displayName: '',
  onDisplayNameChange: () => {},
  url: '',
  onUrlChange: () => {},
  enabled: true,
  onEnabledChange: () => {},
  isPending: false,
  errorMessage: null,
  onSubmit: () => {},
}

const meta = {
  title: 'Settings/RssFeedDialogView',
  component: RssFeedDialogView,
  parameters: { layout: 'centered' },
} satisfies Meta<typeof RssFeedDialogView>

export default meta
type Story = StoryObj<typeof meta>

export const Create: Story = {
  name: 'shows a blank form for creating an RSS feed.',
  args: baseArgs,
}

export const Edit: Story = {
  name: 'shows an enabled feed with its source locked during editing.',
  args: {
    ...baseArgs,
    feed,
    source: feed.source,
    displayName: feed.display_name,
    url: feed.url,
  },
}

export const Disabled: Story = {
  name: 'shows an edited feed that is excluded from aggregation.',
  args: {
    ...baseArgs,
    feed: { ...feed, enabled: false },
    source: feed.source,
    displayName: feed.display_name,
    url: feed.url,
    enabled: false,
  },
}

export const SaveError: Story = {
  name: 'shows a save error while the form stays open.',
  args: {
    ...baseArgs,
    errorMessage: '保存に失敗しました。入力内容をご確認ください。',
  },
}

export const Saving: Story = {
  name: 'shows the disabled submit button while saving.',
  args: {
    ...baseArgs,
    isPending: true,
  },
}
