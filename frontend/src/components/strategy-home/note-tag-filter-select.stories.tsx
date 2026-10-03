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
  name: 'shows the note list without a tag filter.',
  args: { value: undefined },
}

export const SelectedTag: Story = {
  name: 'shows one selected note tag.',
  args: { value: '架空タグ' },
}
