import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'

const meta = {
  title: 'StrategyHome/NoteTagFilterSelect',
  component: NoteTagFilterSelect,
  args: {
    tags: ['架空タグ', '仮の分類'],
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
  args: { value: '架空タグ' },
}
