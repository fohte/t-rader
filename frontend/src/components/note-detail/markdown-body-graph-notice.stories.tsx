import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownBodyGraphNotice } from '#components/note-detail/markdown-body-graph-notice'

const meta = {
  title: 'NoteDetail/MarkdownBodyGraphNotice',
  component: MarkdownBodyGraphNotice,
  args: { children: '図を表示できませんでした' },
} satisfies Meta<typeof MarkdownBodyGraphNotice>

export default meta
type Story = StoryObj<typeof meta>

export const NoticeWithoutRetry: Story = {
  name: 'shows a message when a referenced graph is unavailable.',
}

export const NoticeWithRetry: Story = {
  name: 'shows a retry action after graph rendering fails.',
  args: { onRetry: () => {} },
}
