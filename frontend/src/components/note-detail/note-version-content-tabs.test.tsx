import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type NoteVersionContentTab,
  NoteVersionContentTabs,
  resolveNoteVersionContentTab,
} from '#components/note-detail/note-version-content-tabs'

const resolverCases: {
  name: string
  isReviewing: boolean
  selectedTab: NoteVersionContentTab | null
  expected: NoteVersionContentTab
}[] = [
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
  {
    name: 'keeps the explicitly selected tab while reviewing',
    isReviewing: true,
    selectedTab: 'full',
    expected: 'full',
  },
]

afterEach(() => {
  cleanup()
})

describe('NoteVersionContentTabs', () => {
  it.each(resolverCases)('$name', ({ isReviewing, selectedTab, expected }) => {
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

    await user.click(screen.getByRole('tab', { name: '全文' }))

    rerender(<NoteVersionContentTabs activeTab="full" onChange={onChange} />)

    function readRenderedState() {
      return {
        onChangeCalls: onChange.mock.calls,
        selectedTabs: Array.from(screen.getAllByRole('tab')).map((tab) =>
          tab.getAttribute('aria-selected'),
        ),
      }
    }

    expect(readRenderedState()).toEqual({
      onChangeCalls: [['full']],
      selectedTabs: ['false', 'true'],
    })
  })
})
