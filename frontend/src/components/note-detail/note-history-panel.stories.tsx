import type { Meta, StoryObj } from '@storybook/react-vite'

import { NoteHistoryPanel } from '#components/note-detail/note-history-panel'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'

type ChangeHistory = components['schemas']['ChangeHistory']

const versions: NoteVersion[] = [
  {
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
  },
  {
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
  },
]

const history: ChangeHistory[] = [
  {
    id: 'fixture-history-approval',
    target_kind: 'note',
    target_id: 'fixture-note',
    actor_kind: 'human',
    actor_label: '自分',
    op: 'status_change',
    diff_json: {
      from: 'unread',
      to: 'approved',
      version_id: 'fixture-version-current',
    },
    summary: null,
    created_at: '2026-10-10T12:00:00Z',
  },
]

const meta = {
  title: 'NoteDetail/NoteHistoryPanel',
  component: NoteHistoryPanel,
  parameters: { layout: 'padded' },
  args: {
    versions,
    history,
    isHistoryPending: false,
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

export const LoadingHistory: Story = {
  name: 'shows a loading state while the note history is loading.',
  args: { history: [], isHistoryPending: true },
}

export const Empty: Story = {
  name: 'shows an empty state when the note has no versions.',
  args: { versions: [], history: [] },
}
