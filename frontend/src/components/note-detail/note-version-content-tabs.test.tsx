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
  hasPreviousVersion: boolean
  selectedTab: NoteVersionContentTab | null
  expected: NoteVersionContentTab
}[] = [
  {
    name: 'later versions start with changes',
    hasPreviousVersion: true,
    selectedTab: null,
    expected: 'changes',
  },
  {
    name: 'the first version starts with full text',
    hasPreviousVersion: false,
    selectedTab: null,
    expected: 'full',
  },
  {
    name: 'keeps the explicitly selected tab for later versions',
    hasPreviousVersion: true,
    selectedTab: 'full',
    expected: 'full',
  },
]

afterEach(() => {
  cleanup()
})

describe('NoteVersionContentTabs', () => {
  it.each(resolverCases)(
    '$name',
    ({ hasPreviousVersion, selectedTab, expected }) => {
      expect(
        resolveNoteVersionContentTab(hasPreviousVersion, selectedTab),
      ).toBe(expected)
    },
  )

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
