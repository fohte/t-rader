import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  NoteVersionContentTabs,
  resolveNoteVersionContentTab,
} from '#components/note-detail/note-version-content-tabs'

afterEach(() => {
  cleanup()
})

describe('NoteVersionContentTabs', () => {
  it.each([
    {
      name: 'review starts with changes',
      isReviewing: true,
      selectedTab: null,
      expected: 'changes',
    },
    {
      name: 'ordinary reading starts with full text',
      isReviewing: false,
      selectedTab: null,
      expected: 'full',
    },
  ])('$name', ({ isReviewing, selectedTab, expected }) => {
    expect(resolveNoteVersionContentTab(isReviewing, selectedTab)).toBe(
      expected,
    )
  })

  it('keeps the selected tab and reports tab changes', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const { rerender } = render(
      <NoteVersionContentTabs activeTab="changes" onChange={onChange} />,
    )

    expect(resolveNoteVersionContentTab(true, 'full')).toBe('full')

    await user.click(screen.getByRole('tab', { name: '全文' }))

    expect(onChange.mock.calls).toEqual([['full']])

    rerender(<NoteVersionContentTabs activeTab="full" onChange={onChange} />)

    expect(
      Array.from(screen.getAllByRole('tab')).map((tab) =>
        tab.getAttribute('aria-selected'),
      ),
    ).toEqual(['false', 'true'])
  })
})
