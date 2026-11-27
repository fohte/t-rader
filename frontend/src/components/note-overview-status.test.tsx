import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import {
  approvedNoteOverview,
  currentAndUnreadNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { NoteOverviewStatus } from '#components/note-overview-status'

describe('NoteOverviewStatus', () => {
  it.each([
    [pendingNoteOverview, '未レビュー現行なし未読 1 バージョン'],
    [currentAndUnreadNoteOverview, '承認済み未読 1 バージョン'],
    [approvedNoteOverview, '承認済み'],
  ])('shows the representative note state', (overview, expected) => {
    const { container } = render(<NoteOverviewStatus overview={overview} />)

    expect(container.textContent).toBe(expected)
  })
})
