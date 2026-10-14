import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { GroupDetailPage } from '#components/group-detail-page'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const group: components['schemas']['Group'] = {
  key: 'demo-group',
  name: '見本グループ',
  description: '架空の分類に含まれる銘柄をまとめています。',
  axis: { key: 'demo-axis', name: '見本分類' },
  stocks: [
    { id: 'DEMO-A', name: '架空電機' },
    { id: 'DEMO-B', name: '見本素材' },
  ],
}

const notes: Note[] = [
  {
    id: 'demo-note-group',
    version_id: 'demo-version-group',
    version_no: 1,
    is_current: true,
    title: '分類を確認するメモ',
    body_md: '表示用の架空ノートです。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: '2020-02-03T00:00:00Z',
    updated_at: '2020-02-03T00:00:00Z',
  },
]

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/stocks/$stockId', '/notes', '/notes/$noteId'] },
      )}
    />
  )
}

const meta = {
  title: 'References/GroupDetailPage',
  component: GroupDetailPage,
  decorators: [withRouter],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof GroupDetailPage>

export default meta
type Story = StoryObj<typeof meta>

const loadedArgs = {
  state: 'loaded',
  group,
  heldStockIds: ['DEMO-A'],
  notes,
  isNotesError: false,
} satisfies Story['args']

export const Loaded: Story = {
  name: 'the page shows group details, holdings, and related notes.',
  args: loadedArgs,
}

export const NotFound: Story = {
  name: 'the page explains when a group does not exist.',
  args: { state: 'not-found', axisKey: 'demo-axis', groupKey: 'missing-group' },
}

export const Loading: Story = {
  name: 'the page shows a placeholder while the group loads.',
  args: { state: 'loading', axisKey: 'demo-axis', groupKey: 'demo-group' },
}

export const Error: Story = {
  name: 'the page explains when the group cannot be loaded.',
  args: { state: 'error', axisKey: 'demo-axis', groupKey: 'demo-group' },
}

export const NoStocks: Story = {
  name: 'the page shows an empty state when the group has no stocks.',
  args: {
    ...loadedArgs,
    group: { ...group, stocks: [] },
  },
}

export const NoDescription: Story = {
  name: 'the page omits the description when none is registered.',
  args: {
    ...loadedArgs,
    group: { ...group, description: null },
  },
}

export const NoHoldings: Story = {
  name: 'the page omits holding badges when no stocks are held.',
  args: { ...loadedArgs, heldStockIds: [] },
}

export const NoRelatedNotes: Story = {
  name: 'the page explains when no notes reference the group.',
  args: { ...loadedArgs, notes: [] },
}

export const RelatedNotesLoading: Story = {
  name: 'the page shows placeholders while related notes load.',
  args: { ...loadedArgs, notes: null },
}

export const RelatedNotesError: Story = {
  name: 'the page explains when related notes cannot be loaded.',
  args: { ...loadedArgs, notes: null, isNotesError: true },
}
