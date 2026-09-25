import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionChatAction } from '#components/note-detail/note-version-chat-action'

const meta = {
  title: 'NoteDetail/NoteVersionChatAction',
  component: NoteVersionChatAction,
  parameters: { layout: 'padded' },
  args: {
    title: '架空データの確認',
    onAsk: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-5xl bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionChatAction>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows an action to ask about the displayed note version.',
}
