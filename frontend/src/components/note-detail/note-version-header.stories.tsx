import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'

const baseVersion: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 3,
  title: '架空データの確認メモ',
  body_md: '変更された本文です。',
  frontmatter_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: '追加資料の確認結果を追記しました。',
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

const meta = {
  title: 'NoteDetail/NoteVersionHeader',
  component: NoteVersionHeader,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="max-w-3xl border border-border bg-card p-5 text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionHeader>

export default meta
type Story = StoryObj<typeof meta>

export const PendingReview: Story = {
  name: 'shows the header for a version awaiting review.',
  args: { version: baseVersion },
}

export const CurrentApproved: Story = {
  name: 'shows the header for the current approved version.',
  args: {
    version: {
      ...baseVersion,
      status: 'approved',
      is_current: true,
      change_reason: null,
      reviewed_at: '2026-01-12T03:00:00Z',
    },
  },
}
