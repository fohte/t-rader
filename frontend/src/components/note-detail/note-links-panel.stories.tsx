import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteLinksPanelView } from '#components/note-detail/note-links-panel'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteLinkItem = components['schemas']['NoteLinkItem']

const outgoing: NoteLinkItem[] = [
  {
    note_id: '00000000-0000-0000-0000-000000000101',
    version_id: '00000000-0000-0000-0000-000000000201',
    version_no: 2,
    title: '架空銘柄の購入判断',
  },
  {
    note_id: '00000000-0000-0000-0000-000000000102',
    version_id: null,
    version_no: 4,
    title: '市場環境の観察',
  },
]

const incoming: NoteLinkItem[] = [
  {
    note_id: '00000000-0000-0000-0000-000000000103',
    version_id: '00000000-0000-0000-0000-000000000203',
    version_no: 3,
    title: '売買方針の記録',
  },
]

const meta = {
  title: 'NoteDetail/NoteLinksPanel',
  component: NoteLinksPanelView,
  parameters: { layout: 'padded' },
  args: {
    outgoing: [],
    incoming: [],
    isPending: false,
    isError: false,
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <div className="max-w-md bg-background p-5 text-foreground">
              <Story />
            </div>
          ),
          { paths: ['/notes/$noteId'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof NoteLinksPanelView>

export default meta
type Story = StoryObj<typeof meta>

export const Linked: Story = {
  args: { outgoing, incoming },
}

export const Empty: Story = {}

export const Loading: Story = {
  args: { isPending: true },
}

export const Error: Story = {
  args: { isError: true },
}
