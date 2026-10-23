import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteKindFilterSelect } from '#components/strategy-home/note-kind-filter-select'
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
  title: 'StrategyHome/NoteKindFilterSelect',
  component: NoteKindFilterSelect,
  args: {
    noteKinds,
    catalogState: 'loaded',
    onChange: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-sm">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteKindFilterSelect>

export default meta
type Story = StoryObj<typeof meta>

export const AllKinds: Story = {
  name: 'The selector has no active kind filter.',
  args: { value: undefined },
}

export const SelectedKind: Story = {
  name: 'The selector shows the selected display name.',
  args: { value: 'sample-kind' },
}

export const LongKind: Story = {
  name: 'The selector shows a long selected display name.',
  args: { value: 'sample-long-kind' },
}

export const Loading: Story = {
  name: 'The selected kind remains while the catalog is loading.',
  args: { value: 'missing-kind', catalogState: 'loading' },
}

export const Failed: Story = {
  name: 'The selected kind remains when the catalog request fails.',
  args: { value: 'missing-kind', catalogState: 'error' },
}

export const Unregistered: Story = {
  name: 'The selector keeps an unregistered kind without showing its key.',
  args: { value: 'unlisted-kind' },
}

export const OptionsOpen: Story = {
  name: 'shows available display names in the selector.',
  args: { value: undefined, defaultOpen: true },
}
