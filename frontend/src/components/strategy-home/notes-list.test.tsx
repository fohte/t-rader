import { RouterProvider } from '@tanstack/react-router'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ReviewQueueItem } from '#components/review/review-queue-model'
import { reviewQueueModel } from '#components/review/review-queue-model'
import { NotesList } from '#components/strategy-home/notes-list'
import {
  type NoteOverview,
  type NoteOverviewView,
} from '#components/strategy-home/notes-overview-model'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']
type NoteVersion = components['schemas']['NoteVersion']
type NoteKind = components['schemas']['NoteKind']

const noteKind: NoteKind = {
  key: 'sample-kind',
  display_name: '架空の種別表示名',
  requires_approval: true,
  sort_order: 0,
}

function makeNote(overrides: Partial<Note> = {}): Note {
  return {
    id: 'fake-note-a',
    version_id: 'fake-version-a',
    version_no: 2,
    is_current: true,
    title: '架空の確認ノート',
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: ['架空タグ'],
    kind: 'sample-kind',
    status: 'approved',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'human',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeVersion(overrides: Partial<NoteVersion> = {}): NoteVersion {
  return {
    id: 'fake-version-a',
    note_id: 'fake-note-a',
    version_no: 2,
    is_current: true,
    title: '架空の確認ノート',
    body_md: '架空の本文です。',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    status: 'approved',
    created_by_kind: 'human',
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
    warning_dispositions: overrides.warning_dispositions ?? [],
  }
}

function makeOverview(
  noteOverrides: Partial<Note> = {},
  options: {
    current?: NoteVersion | null
    latestUnread?: NoteVersion | null
    unreadVersionCount?: number
  } = {},
): NoteOverview {
  const representative = makeNote(noteOverrides)
  return {
    representative,
    current:
      options.current === undefined
        ? makeVersion({
            id: representative.version_id,
            note_id: representative.id,
            version_no: representative.version_no,
            title: representative.title,
            status: representative.status,
            is_current: representative.is_current,
          })
        : options.current,
    latest_unread: options.latestUnread ?? null,
    unread_version_count: options.unreadVersionCount ?? 0,
  }
}

const representative = makeVersion()
const latestUnread = makeVersion({
  id: 'fake-version-unread',
  version_no: 4,
  is_current: false,
  status: 'unread',
  created_by_kind: 'llm',
})
const overview = makeOverview(
  { status: 'approved' },
  { current: representative, latestUnread, unreadVersionCount: 2 },
)

function getOverviewRowOutput(titleLink: HTMLElement, unreadLink: HTMLElement) {
  const row = screen.getByRole('listitem')
  return {
    rowCount: screen.getAllByRole('listitem').length,
    titleHref: titleLink.getAttribute('href'),
    unreadHref: unreadLink.getAttribute('href'),
    rowMetadata: Array.from(row.querySelectorAll('[data-slot="chip"]')).map(
      (chip) => chip.textContent,
    ),
    currentStatus: screen.getByText('承認済み').textContent,
  }
}

function getNoteNavigationOutput(search: unknown, state: unknown) {
  return {
    search,
    navigation: reviewQueueModel.readNavigationState(state),
  }
}

function getRejectedRepresentativeOutput() {
  return {
    versionLabel: screen.getByText('代表 v3 · 非現行').textContent,
    status: screen.getByText('却下').textContent,
    currentLabel: screen.getByText('現行なし').textContent,
    kindLabel: screen.getByText('種別なし').textContent,
  }
}

function getCatalogStateOutput(label: string) {
  return {
    label: screen.getByText(label).textContent,
    rawKeyVisible: screen.queryByText('fabricated-kind-key') != null,
  }
}

function getSelectedSnapshotOutput() {
  return {
    selectedVersion: screen.getByText('v3 を選択中').textContent,
    selectionNote: screen.getByText('承認対象は選択時の v3 です。').textContent,
    approvalButtons: screen.getAllByRole('button', {
      name: '選択した 1 件を承認',
    }).length,
  }
}

function getLoadingStateOutput() {
  const status = screen.getByRole('status')
  return {
    busy: status.getAttribute('aria-busy'),
    rowCount: screen.queryAllByRole('listitem').length,
  }
}

function getErrorStateOutput() {
  const alert = screen.getByRole('alert')
  return { role: alert.getAttribute('role'), text: alert.textContent }
}

function getPendingEmptyStateOutput() {
  return {
    rowCount: screen.queryAllByRole('listitem').length,
    message: screen.getByText('未読・レビュー待ちのノートはありません。')
      .textContent,
  }
}

async function renderList(
  overviews: NoteOverview[],
  options: {
    view?: NoteOverviewView
    state?: 'loading' | 'error' | 'loaded'
    noteKinds?: NoteKind[]
    noteKindCatalogState?: 'loading' | 'error' | 'loaded'
    selectedItemIds?: string[]
    selectedItems?: ReviewQueueItem[]
    hasApprovalError?: boolean
    onToggleItem?: (itemKey: string, checked: boolean) => void
    onApproveSelected?: () => void
  } = {},
) {
  const router = createStoryRouter(
    () => (
      <NotesList
        overviews={overviews}
        view={options.view ?? 'all'}
        kind="sample-kind"
        tag="sample-tag"
        state={options.state ?? 'loaded'}
        noteKinds={options.noteKinds ?? [noteKind]}
        noteKindCatalogState={options.noteKindCatalogState ?? 'loaded'}
        selectedItemIds={options.selectedItemIds ?? []}
        selectedItems={options.selectedItems ?? []}
        isApproving={false}
        hasApprovalError={options.hasApprovalError ?? false}
        onToggleItem={options.onToggleItem ?? (() => {})}
        onApproveSelected={options.onApproveSelected ?? (() => {})}
      />
    ),
    { paths: ['/notes/$noteId'] },
  )
  render(<RouterProvider router={router} />)
  await screen.findByRole('heading', { name: 'ノート一覧' })
  return router
}

afterEach(cleanup)

describe('NotesList', () => {
  it('shows one row and separate links for the representative and latest unread version', async () => {
    const user = userEvent.setup()
    const router = await renderList([overview])
    const titleLink = screen.getByRole('link', { name: '架空の確認ノート' })
    const unreadLink = screen.getByRole('link', {
      name: '最新未読 v4 を確認・レビュー',
    })

    expect(getOverviewRowOutput(titleLink, unreadLink)).toEqual({
      rowCount: 1,
      titleHref:
        '/notes/fake-note-a?version=2&version_id=fake-version-a&view=all&kind=sample-kind&tag=sample-tag',
      unreadHref:
        '/notes/fake-note-a?version=4&version_id=fake-version-unread&view=all&kind=sample-kind&tag=sample-tag',
      rowMetadata: [
        '架空の種別表示名',
        '現行 v2',
        '未読 2 バージョン',
        '架空タグ',
      ],
      currentStatus: '承認済み',
    })

    await user.click(titleLink)
    expect(router.state.location.search).toEqual({
      version: 2,
      version_id: 'fake-version-a',
      view: 'all',
      kind: 'sample-kind',
      tag: 'sample-tag',
    })
  })

  it('opens the unread version as the row target in the pending view', async () => {
    const router = await renderList([overview], { view: 'pending' })
    const titleLink = screen.getByRole('link', { name: '架空の確認ノート' })

    await userEvent.click(titleLink)

    expect(
      getNoteNavigationOutput(
        router.state.location.search,
        router.state.location.state,
      ),
    ).toEqual({
      search: {
        version: 4,
        version_id: 'fake-version-unread',
        view: 'pending',
        kind: 'sample-kind',
        tag: 'sample-tag',
      },
      navigation: {
        snapshot: { itemKeys: ['note:fake-version-unread'] },
        currentItemKey: 'note:fake-version-unread',
        items: [
          {
            kind: 'note',
            id: 'fake-version-unread',
            noteId: 'fake-note-a',
            versionNo: 4,
            title: '架空の確認ノート',
            createdAt: '2026-01-01T00:00:00Z',
            noteKind: 'sample-kind',
            createdByKind: 'llm',
            unreadVersionCount: 2,
          },
        ],
        scope: 'notes',
      },
    })
  })

  it('shows a rejected representative separately from the missing current version', async () => {
    await renderList([
      makeOverview(
        {
          id: 'fake-note-b',
          version_id: 'fake-version-b',
          version_no: 3,
          is_current: false,
          status: 'rejected',
          kind: null,
          tags: [],
        },
        { current: null },
      ),
    ])

    expect(getRejectedRepresentativeOutput()).toEqual({
      versionLabel: '代表 v3 · 非現行',
      status: '却下',
      currentLabel: '現行なし',
      kindLabel: '種別なし',
    })
  })

  it.each([
    {
      state: 'loading' as const,
      label: '種別を読み込み中',
      noteKinds: [noteKind],
    },
    {
      state: 'error' as const,
      label: '種別を取得できません',
      noteKinds: [noteKind],
    },
    { state: 'loaded' as const, label: '未登録の種別', noteKinds: [] },
  ])(
    'does not expose an unregistered kind key when the catalog is $state',
    async ({ state, label, noteKinds }) => {
      await renderList([makeOverview({ kind: 'fabricated-kind-key' })], {
        noteKinds,
        noteKindCatalogState: state,
      })

      expect(getCatalogStateOutput(label)).toEqual({
        label,
        rawKeyVisible: false,
      })
    },
  )

  it('passes the note selection key and checked state to its callback', async () => {
    const onToggleItem = vi.fn()
    await renderList([overview], { onToggleItem })

    fireEvent.click(
      screen.getByRole('checkbox', {
        name: '架空の確認ノート の最新未読バージョンを選択',
      }),
    )

    expect(onToggleItem.mock.calls).toEqual([['note:fake-note-a', true]])
  })

  it('shows the selected snapshot version and its approval action', async () => {
    const selectedItem: ReviewQueueItem = {
      kind: 'note',
      id: 'fake-version-older-unread',
      noteId: 'fake-note-a',
      versionNo: 3,
      title: '架空の確認ノート',
      createdAt: '2026-01-01T00:00:00Z',
      noteKind: 'sample-kind',
      createdByKind: 'llm',
      unreadVersionCount: 2,
    }
    await renderList([overview], {
      selectedItemIds: ['note:fake-note-a'],
      selectedItems: [selectedItem],
    })

    expect(getSelectedSnapshotOutput()).toEqual({
      selectedVersion: 'v3 を選択中',
      selectionNote: '承認対象は選択時の v3 です。',
      approvalButtons: 2,
    })
  })

  it('shows the loading state while overviews are being fetched', async () => {
    await renderList([], { state: 'loading' })

    expect(getLoadingStateOutput()).toEqual({ busy: 'true', rowCount: 0 })
  })

  it('shows an error when overviews cannot be fetched', async () => {
    await renderList([], { state: 'error' })

    expect(getErrorStateOutput()).toEqual({
      role: 'alert',
      text: 'ノート一覧を読み込めませんでした。',
    })
  })

  it('shows an empty message when no notes await review', async () => {
    await renderList([], { view: 'pending' })

    expect(getPendingEmptyStateOutput()).toEqual({
      rowCount: 0,
      message: '未読・レビュー待ちのノートはありません。',
    })
  })
})
