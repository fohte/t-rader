import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  approvedNoteOverview,
  currentAndUnreadNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { NoteOverviewStatus } from '#components/note-overview-status'

const meta = {
  title: 'Components/NoteOverviewStatus',
  component: NoteOverviewStatus,
} satisfies Meta<typeof NoteOverviewStatus>

export default meta
type Story = StoryObj<typeof meta>

export const PendingOnly: Story = {
  name: 'shows an unreviewed note without a current version.',
  args: { overview: pendingNoteOverview },
}

export const CurrentAndUnread: Story = {
  name: 'shows an unread version alongside its current version.',
  args: { overview: currentAndUnreadNoteOverview },
}

export const CurrentOnly: Story = {
  name: 'shows a current note without unread versions.',
  args: { overview: approvedNoteOverview },
}
