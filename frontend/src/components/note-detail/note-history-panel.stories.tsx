import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteHistoryPanel } from '#components/note-detail/note-history-panel'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'

type ChangeHistory = components['schemas']['ChangeHistory']

const newVersion: NoteVersion = {
  id: 'fixture-version-new',
  note_id: 'fixture-note',
  version_no: 3,
  title: '架空データの追加確認',
  body_md: '変更箇所です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: '追加資料の確認結果を追記しました。',
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-10-13T11:00:00Z',
  reviewed_at: null,
}

const currentVersion: NoteVersion = {
  id: 'fixture-version-current',
  note_id: 'fixture-note',
  version_no: 2,
  title: '架空データの確認',
  body_md: '以前の本文です。',
  frontmatter_json: {},
  resolved_price_references_json: {},
  graphs_json: [],
  status: 'approved',
  is_current: true,
  change_reason: null,
  created_by_kind: 'human',
  execution_id: null,
  created_at: '2026-10-10T11:00:00Z',
  reviewed_at: '2026-10-10T12:00:00Z',
}

const versions: NoteVersion[] = [newVersion, currentVersion]

const approvalHistory: ChangeHistory = {
  id: 'fixture-history-approval',
  target_kind: 'note',
  target_id: 'fixture-note',
  actor_kind: 'human',
  actor_label: '自分',
  op: 'status_change',
  diff_json: {
    from: 'unread',
    to: 'approved',
    version_id: currentVersion.id,
  },
  summary: null,
  created_at: '2026-10-10T12:00:00Z',
}

const history: ChangeHistory[] = [approvalHistory]

const meta = {
  title: 'NoteDetail/NoteHistoryPanel',
  component: NoteHistoryPanel,
  parameters: { layout: 'padded' },
  args: {
    versions,
    history,
    isHistoryPending: false,
    isHistoryError: false,
    selectedVersionNo: 3,
    onSelectVersion: () => {},
  },
  decorators: [
    (Story) => (
      <div className="max-w-md bg-background text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof NoteHistoryPanel>

export default meta
type Story = StoryObj<typeof meta>

export const SelectedUnreadVersion: Story = {
  name: 'shows the selected unread version and its approval history.',
}

export const SelectedCurrentVersion: Story = {
  name: 'shows the selected current version and its approval history.',
  args: { selectedVersionNo: 2 },
}

export const RejectedVersion: Story = {
  name: 'shows the rejection event for a rejected version.',
  args: {
    versions: versions.map((version) =>
      version.id === 'fixture-version-new'
        ? { ...version, status: 'rejected' as const }
        : version,
    ),
    history: [
      {
        ...approvalHistory,
        id: 'fixture-history-rejection',
        diff_json: {
          from: 'unread',
          to: 'rejected',
          version_id: 'fixture-version-new',
        },
        summary: '確認事項が残っています。',
        created_at: '2026-10-13T12:00:00Z',
      },
    ],
  },
}

export const SupersededVersion: Story = {
  name: 'shows an older version superseded by a newer approval.',
  args: {
    versions: [
      { ...newVersion, status: 'approved', is_current: true },
      { ...currentVersion, status: 'superseded', is_current: false },
    ],
    history: [
      {
        ...approvalHistory,
        id: 'fixture-history-supersession',
        diff_json: {
          from: 'unread',
          to: 'approved',
          version_id: 'fixture-version-new',
          superseded_version_ids: ['fixture-version-current'],
        },
        summary: null,
        created_at: '2026-10-13T12:00:00Z',
      },
    ],
  },
}

export const LoadingHistory: Story = {
  name: 'shows a loading state while the note history is loading.',
  args: { history: [], isHistoryPending: true },
}

export const HistoryError: Story = {
  name: 'shows an error when the note history could not be loaded.',
  args: { history: [], isHistoryError: true },
}

export const Empty: Story = {
  name: 'shows an empty state when the note has no versions.',
  args: { versions: [], history: [] },
}
