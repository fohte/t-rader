import { RouterProvider } from '@tanstack/react-router'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  ReviewPageView,
  type ReviewPageViewProps,
} from '#components/review/review-page-view'
import { reviewQueueModel } from '#components/review/review-queue-model'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteKind = components['schemas']['NoteKind']

const noteKinds: NoteKind[] = [
  {
    key: 'fabricated-kind-key',
    display_name: '架空の種別表示名',
    description: null,
    requires_approval: false,
    sort_order: 1,
  },
]

const props: ReviewPageViewProps = {
  data: {
    counts: { notes: 1, annotations: 0, total: 1 },
    items: [
      {
        kind: 'note',
        id: 'fake-version-a',
        noteId: 'fake-note-a',
        versionNo: 1,
        title: '架空の確認メモ',
        createdAt: '2026-01-12T02:00:00Z',
        noteKind: 'fabricated-kind-key',
        createdByKind: 'llm',
        unreadVersionCount: 1,
      },
    ],
  },
  noteKinds,
  navigationSnapshot: null,
  isPending: false,
  isError: false,
  typeFilter: 'all',
  noteKindFilter: 'all',
  selectedItemIds: [],
  isApproving: false,
  hasApprovalError: false,
  onTypeFilterChange: () => {},
  onNoteKindFilterChange: () => {},
  onToggleItem: () => {},
  onToggleVisibleItems: () => {},
  onApproveSelected: () => {},
}

afterEach(cleanup)

function getKindPresentation(kindOption: HTMLElement) {
  return {
    displayNames: screen
      .getAllByText('架空の種別表示名')
      .map((element) => element.textContent),
    rawKeyText:
      screen.queryByText('fabricated-kind-key', { exact: true })?.textContent ??
      null,
    filterSelected: kindOption.getAttribute('aria-selected'),
  }
}

function renderReviewPage(overrides: Partial<ReviewPageViewProps> = {}) {
  const router = createStoryRouter(
    () => <ReviewPageView {...props} {...overrides} />,
    { paths: ['/notes/$noteId', '/annotations/$annoId'] },
  )
  render(<RouterProvider router={router} />)
}

describe('ReviewPageView', () => {
  it('opens a reviewed note version in its note detail route', async () => {
    renderReviewPage()
    const noteLink = await screen.findByRole('link', {
      name: /架空の確認メモ/,
    })

    expect(noteLink.getAttribute('href')).toEqual(
      '/notes/fake-note-a?version=1',
    )
  })

  it('shows the display name for the selected note kind', async () => {
    renderReviewPage({
      typeFilter: 'note',
      noteKindFilter: reviewQueueModel.noteKindFilterKey('fabricated-kind-key'),
      noteKindFilterOpen: true,
    })
    const kindOption = await screen.findByRole('option', {
      name: '架空の種別表示名',
    })

    expect(getKindPresentation(kindOption)).toEqual({
      displayNames: [
        '架空の種別表示名',
        '架空の種別表示名',
        '架空の種別表示名',
      ],
      rawKeyText: null,
      filterSelected: 'true',
    })
  }, 15_000)

  it('passes the selected note kind to its filter callback', async () => {
    const user = userEvent.setup()
    const onNoteKindFilterChange = vi.fn()
    renderReviewPage({
      typeFilter: 'note',
      noteKindFilterOpen: true,
      onNoteKindFilterChange,
    })

    await user.click(
      await screen.findByRole('option', { name: '架空の種別表示名' }),
    )

    expect(onNoteKindFilterChange.mock.calls).toEqual([
      [reviewQueueModel.noteKindFilterKey('fabricated-kind-key')],
    ])
  })

  it('passes the selected item type to its filter callback', async () => {
    const onTypeFilterChange = vi.fn()
    renderReviewPage({ onTypeFilterChange })

    fireEvent.click(await screen.findByRole('button', { name: 'ノート 1' }))

    expect(onTypeFilterChange.mock.calls).toEqual([['note']])
  })

  it('passes the selected item key and checked state to its callback', async () => {
    const onToggleItem = vi.fn()
    renderReviewPage({ onToggleItem })

    fireEvent.click(
      await screen.findByRole('checkbox', {
        name: 'レビュー項目を選択: 架空の確認メモ',
      }),
    )

    expect(onToggleItem.mock.calls).toEqual([['note:fake-version-a', true]])
  })

  it('passes all visible item keys when selecting every item', async () => {
    const user = userEvent.setup()
    const onToggleVisibleItems = vi.fn()
    renderReviewPage({
      data: {
        counts: { notes: 2, annotations: 0, total: 2 },
        items: [
          ...(props.data?.items ?? []),
          {
            kind: 'note',
            id: 'fake-version-b',
            noteId: 'fake-note-b',
            versionNo: 1,
            title: '別の架空メモ',
            createdAt: '2026-01-11T02:00:00Z',
            noteKind: 'fabricated-kind-key',
            createdByKind: 'llm',
            unreadVersionCount: 1,
          },
        ],
      },
      onToggleVisibleItems,
    })

    await user.click(await screen.findByLabelText('表示中の項目をすべて選択'))

    expect(onToggleVisibleItems.mock.calls).toEqual([
      [['note:fake-version-a', 'note:fake-version-b'], true],
    ])
  })

  it('sends the approval action through its callback', async () => {
    const onApproveSelected = vi.fn()
    renderReviewPage({
      selectedItemIds: ['note:fake-version-a'],
      onApproveSelected,
    })

    const [approvalButton] = await screen.findAllByRole('button', {
      name: '選択した 1 件を承認',
    })
    if (approvalButton == null) throw new Error('approval button is missing')
    fireEvent.click(approvalButton)

    expect(onApproveSelected.mock.calls.length).toEqual(1)
  })
})
