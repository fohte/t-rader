import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteKind = components['schemas']['NoteKind']

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

const noteKinds: NoteKind[] = [
  {
    key: 'sample-kind',
    display_name: '架空の種別表示名',
    requires_approval: false,
    sort_order: 0,
  },
  {
    key: 'sample-long-kind',
    display_name: '長い架空の種別表示名を折り返して読める状態',
    requires_approval: false,
    sort_order: 1,
  },
]

const meta = {
  title: 'NoteDetail/NoteVersionHeader',
  component: NoteVersionHeader,
  parameters: { layout: 'padded' },
  args: {
    noteKind: 'sample-kind',
    noteKinds,
    noteKindCatalogState: 'loaded',
    noteKindSourceState: 'available',
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
            paths: [
              '/notes',
              '/strategies/$id',
              '/strategies/$id/runs/$taskId',
            ],
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
  args: {
    version: {
      ...baseVersion,
      title: '生成元を特定できない確認メモ',
      execution_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    },
    executionOrigin: null,
  },
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
    noteKind: null,
  },
}

export const KindUnset: Story = {
  name: 'shows when a note has no kind.',
  args: {
    version: baseVersion,
    noteKind: null,
  },
}

export const KindLoading: Story = {
  name: 'shows while the kind catalog is loading.',
  args: {
    version: baseVersion,
    noteKindCatalogState: 'loading',
  },
}

export const KindUnavailable: Story = {
  name: 'shows when the kind catalog could not be loaded.',
  args: {
    version: baseVersion,
    noteKindCatalogState: 'error',
  },
}

export const NoteKindLoading: Story = {
  name: 'shows while the note kind is loading.',
  args: {
    version: baseVersion,
    noteKindSourceState: 'loading',
  },
}

export const NoteKindUnavailable: Story = {
  name: 'shows when the note kind could not be loaded.',
  args: {
    version: baseVersion,
    noteKindSourceState: 'error',
  },
}

export const KindUnregistered: Story = {
  name: 'shows when the kind is absent from the loaded catalog.',
  args: {
    version: baseVersion,
    noteKind: 'unlisted-kind',
    noteKinds: [],
  },
}

export const LongKind: Story = {
  name: 'shows a long kind label before a long note title.',
  args: {
    version: {
      ...baseVersion,
      title: '架空の長いタイトルを表示しても読みやすく保つメモ',
    },
    noteKind: 'sample-long-kind',
  },
}
