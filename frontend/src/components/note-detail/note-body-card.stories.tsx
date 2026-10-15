import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteBodyCard } from '#components/note-detail/note-body-card'

const meta = {
  component: NoteBodyCard,
} satisfies Meta<typeof NoteBodyCard>

export default meta
type Story = StoryObj<typeof meta>

export const FormattedBody: Story = {
  name: 'the note card aligns with the surrounding page content.',
  decorators: [
    (Story) => (
      <div className="space-y-4 bg-background p-5 text-foreground">
        <div className="text-sm">ノート一覧に戻る</div>
        <Story />
      </div>
    ),
  ],
  args: {
    children: (
      <div>
        <h2 className="mb-2.5 text-lg font-bold">見出し</h2>
        <p className="text-sm leading-relaxed">
          本文の位置はカード内で保たれます。
        </p>
      </div>
    ),
  },
}
