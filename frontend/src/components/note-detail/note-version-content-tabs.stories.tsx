import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionContentTabs } from '#components/note-detail/note-version-content-tabs'

const meta = {
  title: 'NoteDetail/NoteVersionContentTabs',
  component: NoteVersionContentTabs,
  parameters: { layout: 'padded' },
  args: { onChange: () => {} },
  decorators: [
    (Story) => (
      <div className="max-w-3xl border border-border bg-card text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionContentTabs>

export default meta
type Story = StoryObj<typeof meta>

export const ChangesSelected: Story = {
  name: 'shows the changes tab selected by default.',
  args: { activeTab: 'changes' },
}

export const FullTextSelected: Story = {
  name: 'shows the full text tab selected.',
  args: { activeTab: 'full' },
}
