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
  warning_dispositions: [],
  graphs_json: [],
  status: 'unread',
  is_current: false,
  change_reason: '追加資料の確認結果を追記しました。',
  created_by_kind: 'llm',
  execution_id: null,
  created_at: '2026-01-12T11:00:00Z',
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
  warning_dispositions: [],
  graphs_json: [],
  status: 'approved',
  is_current: true,
  change_reason: null,
  created_by_kind: 'human',
  execution_id: null,
  created_at: '2026-01-10T11:00:00Z',
  reviewed_at: '2026-01-10T12:00:00Z',
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
  created_at: '2026-01-10T12:00:00Z',
}

const metadataUpdateHistory: ChangeHistory = {
  ...approvalHistory,
  id: 'fixture-history-metadata-update',
  op: 'update',
  diff_json: { title: { from: '確認前の題名', to: '確認後の題名' } },
  summary: '確認後の題名に更新しました。',
  created_at: '2026-01-12T12:00:00Z',
}

const eventBeforeFirstVersionHistory: ChangeHistory = {
  ...metadataUpdateHistory,
  id: 'fixture-history-before-first-version',
  summary: 'ノート情報を更新しました。',
  created_at: '2026-01-09T12:00:00Z',
}

const currentVersionChangeHistory: ChangeHistory = {
  ...approvalHistory,
  id: 'fixture-history-current-version-change',
  op: 'update',
  diff_json: {
    from_version_id: newVersion.id,
    to_version_id: currentVersion.id,
    version_no: currentVersion.version_no,
  },
  summary: null,
  created_at: '2026-01-11T12:00:00Z',
}

const agentUpdateHistory: ChangeHistory = {
  ...metadataUpdateHistory,
  id: 'fixture-history-agent-update',
  actor_kind: 'llm',
  actor_label: 'エージェント',
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
  name: "shows the selected unread version and the previous version's approval history.",
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
        created_at: '2026-01-12T12:00:00Z',
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
        created_at: '2026-01-12T12:00:00Z',
      },
    ],
  },
}

export const UnversionedEvent: Story = {
  name: 'attaches an update to the newest version created before it.',
  args: { history: [metadataUpdateHistory] },
}

export const EventBeforeFirstVersion: Story = {
  name: 'shows an event older than every listed version at the end.',
  args: { history: [eventBeforeFirstVersionHistory] },
}

export const MultipleEventsOnVersion: Story = {
  name: 'shows multiple events under the same version.',
  args: {
    selectedVersionNo: 2,
    history: [approvalHistory, currentVersionChangeHistory],
  },
}

export const AgentEvent: Story = {
  name: 'shows an update event created by an agent.',
  args: { history: [agentUpdateHistory] },
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
