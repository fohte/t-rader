import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'
import { createStoryRouter } from '#storybook/story-router'

const baseVersion: NoteVersion = {
  id: 'fake-version-a',
  note_id: 'fake-note-a',
  version_no: 3,
  title: '架空データの確認メモ',
  body_md: '変更された本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: '追加資料の確認結果を追記しました。',
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T02:00:00Z',
  reviewed_at: null,
}

const executionOrigin = {
  strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
  strategyName: '架空戦略',
  taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
}

const meta = {
  title: 'NoteDetail/NoteVersionHeader',
  component: NoteVersionHeader,
  parameters: { layout: 'padded' },
  args: {
    noteKindLabel: '架空の種別表示名',
    hasNoteError: false,
    executionOrigin: null,
    onAsk: () => {},
  },
  decorators: [
    (Story) => (
      <RouterProvider
        router={createStoryRouter(
          () => (
            <div className="max-w-3xl border border-border bg-card p-5 text-foreground">
              <Story />
            </div>
          ),
          {
            paths: ['/strategies/$id', '/strategies/$id/runs/$taskId'],
            initialPath: '/strategies/sample-strategy',
          },
        )}
      />
    ),
  ],
} satisfies Meta<typeof NoteVersionHeader>

export default meta
type Story = StoryObj<typeof meta>

export const PendingReview: Story = {
  name: 'shows the metadata and change reason for a version awaiting review.',
  args: { version: baseVersion },
}

export const GeneratedByStrategy: Story = {
  name: 'shows the strategy and run that generated this version.',
  args: {
    version: {
      ...baseVersion,
      execution_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    },
    executionOrigin,
  },
}

export const NoGenerationOrigin: Story = {
  name: 'shows a version without a resolvable generation origin.',
  args: { version: baseVersion, executionOrigin: null },
}

export const HumanCreated: Story = {
  name: 'does not show origin links for a version written by a user.',
  args: {
    version: { ...baseVersion, created_by_kind: 'human', execution_id: null },
    executionOrigin,
  },
}

export const FirstVersion: Story = {
  name: 'shows the full note metadata without a change reason for its first version.',
  args: {
    version: { ...baseVersion, version_no: 1, change_reason: null },
  },
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
    noteKindLabel: null,
  },
}

export const KindUnavailable: Story = {
  name: 'shows when the note kind could not be loaded.',
  args: {
    version: baseVersion,
    hasNoteError: true,
  },
}
