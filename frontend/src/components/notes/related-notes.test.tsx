import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { RelatedNotes } from '#components/notes/related-notes'
import { createStoryRouter } from '#storybook/story-router'

type RelatedNote = ComponentProps<typeof RelatedNotes>['notes'][number]

afterEach(cleanup)

describe('RelatedNotes', () => {
  it('sorts notes by most recent update', async () => {
    const notes: RelatedNote[] = [
      {
        id: 'fake-note-a',
        title: '古いノート',
        status: 'approved',
        strategyName: '架空戦略',
        author: '自分',
        updatedAt: '2026-10-09T03:00:00Z',
      },
      {
        id: 'fake-note-b',
        title: '新しいノート',
        status: 'unread',
        strategyName: '架空戦略',
        author: 'エージェント',
        updatedAt: '2026-10-10T03:00:00Z',
      },
    ]
    const router = createStoryRouter(() => <RelatedNotes notes={notes} />, {
      paths: ['/notes/$noteId'],
    })
    render(<RouterProvider router={router} />)
    const links = await screen.findAllByRole('link')

    expect(
      links.map((link) => ({
        title: link.querySelector('span span')?.textContent,
        href: link.getAttribute('href'),
      })),
    ).toEqual([
      { title: '新しいノート', href: '/notes/fake-note-b' },
      { title: '古いノート', href: '/notes/fake-note-a' },
    ])
  })
})
