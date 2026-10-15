import type { Meta, StoryObj } from '@storybook/react-vite'

import { MarkdownBodyTextSelectionAction } from '#components/note-detail/markdown-body-text-selection-action'

const meta = {
  title: 'NoteDetail/MarkdownBodyTextSelectionAction',
  component: MarkdownBodyTextSelectionAction,
  parameters: { layout: 'fullscreen' },
  args: {
    selection: {
      text: '選択された本文',
      range: { startLine: 3, endLine: 4 },
      position: { top: 64, left: 180 },
    },
    onStartComment: () => {},
  },
} satisfies Meta<typeof MarkdownBodyTextSelectionAction>

export default meta
type Story = StoryObj<typeof meta>

export const SelectionIsReady: Story = {
  name: 'shows a comment action above the selected text.',
}
