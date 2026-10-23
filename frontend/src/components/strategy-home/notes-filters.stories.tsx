import type { Meta, StoryObj } from '@storybook/react-vite'

import { NotesFilters } from '#components/strategy-home/notes-filters'
import type { components } from '#lib/api/schema.gen'

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
    display_name: '長い架空の種別表示名を折り返して表示します',
    requires_approval: false,
    sort_order: 1,
  },
]

const meta = {
  title: 'StrategyHome/NotesFilters',
  component: NotesFilters,
  args: {
    noteKinds,
    catalogState: 'loaded',
    kind: undefined,
    tags: ['架空タグ', '仮の分類'],
    tag: undefined,
    view: 'all',
    onViewChange: () => {},
    onKindChange: () => {},
    onTagChange: () => {},
    onClear: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-sm">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NotesFilters>

export default meta
type Story = StoryObj<typeof meta>

export const Available: Story = {
  name: 'shows both filters and a control for clearing them.',
}

export const Combined: Story = {
  name: 'shows a kind and tag selected together.',
  args: { kind: 'sample-kind', tag: '架空タグ' },
}

export const PendingNotes: Story = {
  name: 'shows the unread and review-waiting view selected.',
  args: { view: 'pending' },
}

export const LongKind: Story = {
  name: 'keeps a long kind label readable in a narrow layout.',
  args: { kind: 'sample-long-kind' },
}

export const CatalogLoading: Story = {
  name: 'keeps the selected kind while the catalog loads.',
  args: { kind: 'unknown-kind', catalogState: 'loading' },
}

export const CatalogFailed: Story = {
  name: 'keeps the selected kind after a catalog failure.',
  args: { kind: 'unknown-kind', catalogState: 'error' },
}

export const UnregisteredKind: Story = {
  name: 'keeps an unregistered kind selected without showing its key.',
  args: { kind: 'unlisted-kind' },
}
