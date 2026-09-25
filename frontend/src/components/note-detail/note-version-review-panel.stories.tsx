import type { Meta, StoryObj } from '@storybook/react-vite'

import {
  NoteVersionReviewView,
  type NoteVersionReviewViewProps,
} from '#components/note-detail/note-version-review-panel'
import type { NoteVersion } from '#lib/api/note-version-types'

const pendingVersion: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 4,
  title: '架空データの確認メモ',
  body_md: '変更された本文です。',
  frontmatter_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: null,
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

const args: NoteVersionReviewViewProps = {
  version: pendingVersion,
  lineCommentCount: 0,
  isCommentsPending: false,
  reason: '',
  isApproving: false,
  isRejecting: false,
  isMakingCurrent: false,
  hasError: false,
  onReasonChange: () => {},
  onApprove: () => {},
  onReject: () => {},
  onMakeCurrent: () => {},
}

const meta = {
  title: 'NoteDetail/NoteVersionReviewView',
  component: NoteVersionReviewView,
  parameters: { layout: 'padded' },
  args,
  decorators: [
    (Story) => (
      <div className="max-w-md bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteVersionReviewView>

export default meta
type Story = StoryObj<typeof meta>

export const ReasonRequiredWithoutComments: Story = {
  name: 'shows the rejection reason marked as required when there are no line comments.',
}

export const CommentAllowsEmptyReason: Story = {
  name: 'shows the rejection reason marked as optional when the review has a line comment.',
  args: { lineCommentCount: 1 },
}

export const RestoringApprovedVersion: Story = {
  name: 'shows the action for making an older approved version current.',
  args: {
    version: { ...pendingVersion, status: 'approved', is_current: false },
  },
}

export const CurrentVersion: Story = {
  name: 'shows the review panel for the current approved version.',
  args: {
    version: { ...pendingVersion, status: 'approved', is_current: true },
  },
}
