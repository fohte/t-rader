import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'

const meta = {
  title: 'StrategyHome/NoteTagFilterSelect',
  component: NoteTagFilterSelect,
  args: {
    noteTags: [
      { key: 'theme-x', display_name: 'テーマX' },
      { key: 'category-y', display_name: '分類Y' },
    ],
    catalogState: 'loaded',
    onChange: () => {},
  },
} satisfies Meta<typeof NoteTagFilterSelect>

export default meta
type Story = StoryObj<typeof meta>

export const AllTags: Story = {
  name: 'The selector has no active tag filter.',
  args: { value: undefined },
}

export const SelectedTag: Story = {
  name: 'The selector has one tag selected.',
  args: { value: 'theme-x' },
}

export const OptionsOpen: Story = {
  name: 'The selector shows registered tags by display name.',
  args: { value: undefined, defaultOpen: true },
}
