import { RouterProvider } from '@tanstack/react-router'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { RelatedNotes } from '#components/related-notes'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Note = components['schemas']['Note']

const notes: Note[] = [
  {
    id: 'demo-note-older',
    version_id: 'demo-version-older',
    version_no: 1,
    is_current: true,
    title: '以前のメモ',
    body_md: '',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'approved',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'human',
    created_at: '2026-10-02T00:00:00Z',
    updated_at: '2026-10-01T00:00:00Z',
  },
  {
    id: 'demo-note-newer',
    version_id: 'demo-version-newer',
    version_no: 1,
    is_current: true,
    title: '新しいメモ',
    body_md: '',
    frontmatter_json: {},
    resolved_price_references_json: {},
    graphs_json: [],
    tags: [],
    kind: null,
    status: 'unread',
    trigger: null,
    trigger_label: null,
    created_by_kind: 'llm',
    created_at: '2026-10-01T00:00:00Z',
    updated_at: '2026-10-02T00:00:00Z',
  },
]

describe('RelatedNotes', () => {
  it('links notes in newest-first order', async () => {
    const router = createStoryRouter(
      () => <RelatedNotes notes={notes} isError={false} />,
      { paths: ['/notes', '/notes/$noteId'] },
    )
    render(<RouterProvider router={router} />)

    const links = await screen.findAllByRole('link')
    expect(links.map((link) => link.getAttribute('href'))).toEqual([
      '/notes/demo-note-newer',
      '/notes/demo-note-older',
    ])
  })
})
