import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

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

function getKindPresentation(
  container: HTMLElement,
  kindOption: HTMLOptionElement,
) {
  return {
    displayNameCount: screen.getAllByText('架空の種別表示名').length,
    rawKeyText: container.textContent.includes('fabricated-kind-key'),
    filterValue: kindOption.getAttribute('value'),
  }
}

describe('ReviewPageView', () => {
  it('shows the kind display name while keeping the key in the filter value', async () => {
    const router = createStoryRouter(() => <ReviewPageView {...props} />, {
      paths: ['/notes/$noteId', '/annotations/$annoId'],
    })
    const { container } = render(<RouterProvider router={router} />)
    const kindOption = await screen.findByRole('option', {
      name: '架空の種別表示名',
    })

    expect(getKindPresentation(container, kindOption)).toEqual({
      displayNameCount: 2,
      rawKeyText: false,
      filterValue: reviewQueueModel.noteKindFilterKey('fabricated-kind-key'),
    })
  })
})
