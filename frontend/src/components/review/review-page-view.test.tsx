import { RouterProvider } from '@tanstack/react-router'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { MouseEvent } from 'react'
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

function getReviewActionOutput(
  typeFilters: readonly unknown[][],
  itemToggles: readonly unknown[][],
  approvals: Array<string | null>,
) {
  return { typeFilters, itemToggles, approvals }
}

describe('ReviewPageView', () => {
  it('shows the display name for the selected note kind', async () => {
    const router = createStoryRouter(
      () => (
        <ReviewPageView
          {...props}
          typeFilter="note"
          noteKindFilter={reviewQueueModel.noteKindFilterKey(
            'fabricated-kind-key',
          )}
          noteKindFilterOpen
        />
      ),
      {
        paths: ['/notes/$noteId', '/annotations/$annoId'],
      },
    )
    render(<RouterProvider router={router} />)
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

  it('sends filter, item selection, and approval actions through callbacks', async () => {
    const onTypeFilterChange = vi.fn()
    const onToggleItem = vi.fn()
    const onApproveSelected =
      vi.fn<(event?: MouseEvent<HTMLButtonElement>) => void>()
    const router = createStoryRouter(
      () => (
        <ReviewPageView
          {...props}
          selectedItemIds={['note:fake-version-a']}
          onTypeFilterChange={onTypeFilterChange}
          onToggleItem={onToggleItem}
          onApproveSelected={onApproveSelected}
        />
      ),
      { paths: ['/notes/$noteId', '/annotations/$annoId'] },
    )
    render(<RouterProvider router={router} />)

    fireEvent.click(await screen.findByRole('button', { name: 'ノート 1' }))
    fireEvent.click(
      await screen.findByRole('checkbox', {
        name: 'レビュー項目を選択: 架空の確認メモ',
      }),
    )
    const [approvalButton] = await screen.findAllByRole('button', {
      name: '選択した 1 件を承認',
    })
    if (approvalButton === undefined)
      throw new Error('approval button is missing')
    fireEvent.click(approvalButton)

    expect(
      getReviewActionOutput(
        onTypeFilterChange.mock.calls,
        onToggleItem.mock.calls,
        onApproveSelected.mock.calls.map(([event]) => event?.type ?? null),
      ),
    ).toEqual({
      typeFilters: [['note']],
      itemToggles: [['note:fake-version-a', false]],
      approvals: ['click'],
    })
  })
})
