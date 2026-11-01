import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteHistoryPanel } from '#components/note-detail/note-history-panel'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'

type ChangeHistory = components['schemas']['ChangeHistory']

const fixtureVersions: NoteVersion[] = [
  {
    id: 'fixture-version-old',
    note_id: 'fixture-note',
    version_no: 2,
    title: '以前の架空ノート',
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
    created_at: '2026-10-10T13:00:00Z',
    reviewed_at: '2026-10-10T14:00:00Z',
  },
  {
    id: 'fixture-version-new',
    note_id: 'fixture-note',
    version_no: 3,
    title: '新しい架空ノート',
    body_md: '新しい本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    warning_dispositions: [],
    graphs_json: [],
    status: 'unread',
    is_current: false,
    change_reason: '確認結果を追記しました。',
    created_by_kind: 'llm',
    execution_id: null,
    created_at: '2026-10-13T14:00:00Z',
    reviewed_at: null,
  },
]

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
    version_id: 'fixture-version-old',
  },
  summary: '記載内容を確認しました。',
  created_at: '2026-10-11T14:00:00Z',
}

const fixtureHistory: ChangeHistory[] = [approvalHistory]

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('NoteHistoryPanel', () => {
  it('shows events under the newest version created before each event', () => {
    vi.useFakeTimers({ toFake: ['Date'] })
    const now = new Date('2026-10-13T15:00:00.000Z').getTime()
    vi.setSystemTime(now)
    const day = 24 * 60 * 60 * 1000
    const versions = fixtureVersions.map((version) => ({
      ...version,
      created_at: new Date(
        now - (version.version_no === 3 ? 6 * 60 * 60 * 1000 : 3 * day),
      ).toISOString(),
    }))
    const history: ChangeHistory[] = [
      ...fixtureHistory.map((entry) => ({
        ...entry,
        created_at: new Date(now - 2 * day).toISOString(),
      })),
      {
        ...approvalHistory,
        id: 'fixture-history-current-change',
        op: 'update',
        diff_json: {
          from_version_id: 'fixture-version-new',
          to_version_id: 'fixture-version-old',
          version_no: 2,
        },
        summary: null,
        created_at: new Date(now - day).toISOString(),
      },
      {
        ...approvalHistory,
        id: 'fixture-history-update-before-new-version',
        op: 'update',
        diff_json: { title: { from: '以前の題名', to: '新しい題名' } },
        summary: '以前の題名を更新しました。',
        created_at: new Date(now - 7 * 60 * 60 * 1000).toISOString(),
      },
      {
        ...approvalHistory,
        id: 'fixture-history-update-after-new-version',
        op: 'update',
        diff_json: { title: { from: '新しい題名', to: '最新の題名' } },
        summary: '最新の題名を更新しました。',
        created_at: new Date(now - 5 * 60 * 60 * 1000).toISOString(),
      },
    ]
    render(
      <NoteHistoryPanel
        versions={versions}
        history={history}
        isHistoryPending={false}
        isHistoryError={false}
        selectedVersionNo={3}
        onSelectVersion={() => {}}
      />,
    )

    const versionRows = Array.from(
      screen
        .getByRole('region', { name: '履歴' })
        .querySelectorAll(':scope > ol > li'),
    ).map((row) => {
      const button = row.querySelector('button')
      const events = row.querySelector(':scope > ul')
      return button == null
        ? { event: row.textContent.replace(/\s+/g, ' ').trim() }
        : {
            version: button.textContent.replace(/\s+/g, ' ').trim(),
            isSelected: button.getAttribute('aria-current') === 'true',
            events: Array.from(events?.querySelectorAll('li') ?? []).map(
              (event) => event.textContent.replace(/\s+/g, ' ').trim(),
            ),
          }
    })

    expect(versionRows).toEqual([
      {
        version: 'v3新しい架空ノート未レビューエージェント · 6 時間前',
        isSelected: true,
        events: ['5 時間前自分が更新最新の題名を更新しました。'],
      },
      {
        version: 'v2現行以前の架空ノート承認済み自分 · 3 日前',
        isSelected: false,
        events: [
          '7 時間前自分が更新以前の題名を更新しました。',
          '昨日自分が現行に変更',
          '2 日前自分が承認記載内容を確認しました。',
        ],
      },
    ])
  })

  it('selects the clicked version', async () => {
    const onSelectVersion = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteHistoryPanel
        versions={fixtureVersions}
        history={fixtureHistory}
        isHistoryPending={false}
        isHistoryError={false}
        selectedVersionNo={3}
        onSelectVersion={onSelectVersion}
      />,
    )

    await user.click(screen.getByRole('button', { name: /v2/ }))

    expect(onSelectVersion.mock.calls).toEqual([[2]])
  })

  it('shows an error when history could not be loaded', () => {
    render(
      <NoteHistoryPanel
        versions={[]}
        history={[]}
        isHistoryPending={false}
        isHistoryError
        selectedVersionNo={3}
        onSelectVersion={() => {}}
      />,
    )

    expect(screen.getByRole('region', { name: '履歴' }).textContent).toBe(
      '履歴バージョンがありません。履歴を読み込めませんでした。',
    )
  })
})
