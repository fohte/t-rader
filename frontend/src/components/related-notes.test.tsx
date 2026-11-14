import { RouterProvider } from '@tanstack/react-router'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import {
  currentAndUnreadNoteOverview,
  pendingNoteOverview,
} from '#components/dashboard/dashboard-notes.fixtures'
import { RelatedNotes } from '#components/related-notes'
import { createStoryRouter } from '#storybook/story-router'

const notes = [pendingNoteOverview, currentAndUnreadNoteOverview]

describe('RelatedNotes', () => {
  it('links notes in newest-first order', async () => {
    const router = createStoryRouter(
      () => <RelatedNotes notes={notes} isError={false} />,
      { paths: ['/notes', '/notes/$noteId'] },
    )
    render(<RouterProvider router={router} />)

    const links = await screen.findAllByRole('link')
    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/notes/fake-note-current-unread',
      '/notes/fake-note-pending',
    ])
  })
})
