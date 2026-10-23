import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteKindLabel } from '#components/notes/note-kind-label'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
  {
    key: 'sample-long-kind',
    display_name: '長い架空の種別表示名を折り返して読める状態',
    requires_approval: false,
    sort_order: 1,
  },
]

const meta = {
  title: 'Notes/NoteKindLabel',
  component: NoteKindLabel,
  args: {
    kind: 'sample-kind',
    noteKinds,
    catalogState: 'loaded',
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <Story />
          ),
          { paths: ['/notes'] },
        )}
      />
    ),
  ],
} satisfies Meta<typeof NoteKindLabel>

export default meta
type Story = StoryObj<typeof meta>

export const Registered: Story = {
  name: 'shows the registered display name.',
}

export const LongName: Story = {
  name: 'shows a long display name without exposing its key.',
  args: { kind: 'sample-long-kind' },
  decorators: [
    (Story) => (
      <div className="max-w-52">
        <Story />
      </div>
    ),
  ],
}

export const Unset: Story = {
  name: 'shows when a note has no kind.',
  args: { kind: null },
}

export const Loading: Story = {
  name: 'shows while the kind catalog is loading.',
  args: { catalogState: 'loading' },
}

export const Failed: Story = {
  name: 'shows when the kind catalog could not be loaded.',
  args: { catalogState: 'error' },
}

export const Unregistered: Story = {
  name: 'shows when a kind is missing from the loaded catalog.',
  args: { kind: 'unlisted-kind', noteKinds: [] },
}
