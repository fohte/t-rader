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

export const CatalogLoading: Story = {
  name: 'The selector shows a loading label while the tag vocabulary loads.',
  args: { value: 'theme-x', catalogState: 'loading' },
}

export const CatalogFailed: Story = {
  name: 'The selector shows an error label when the vocabulary cannot load.',
  args: { value: 'theme-x', catalogState: 'error' },
}

export const UnregisteredTag: Story = {
  name: 'The selector hides the key of an unregistered selected tag.',
  args: { value: 'unlisted-tag' },
}

export const OptionsOpen: Story = {
  name: 'The selector shows registered tags by display name.',
  args: { value: undefined, defaultOpen: true },
}
