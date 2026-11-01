import { Outlet, RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { NotesTabs } from '#components/notes/notes-tabs'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('NotesTabs', () => {
  it('switches tabs in the URL while preserving note filters and the view', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(() => <Outlet />, {
      paths: [
        {
          path: '/notes',
          component: () => <NotesTabs activeTab="notes" />,
        },
      ],
      initialPath: '/notes?view=pending&kind=sample-kind&tag=sample-tag',
    })
    render(<RouterProvider router={router} />)

    const outputs: { pathname: string; search: Record<string, unknown> }[] = []
    await user.click(await screen.findByRole('tab', { name: 'アノテーション' }))
    outputs.push({
      pathname: router.state.location.pathname,
      search: router.state.location.search,
    })

    await user.click(await screen.findByRole('tab', { name: 'ノート' }))
    outputs.push({
      pathname: router.state.location.pathname,
      search: router.state.location.search,
    })

    expect(outputs).toEqual([
      {
        pathname: '/notes',
        search: {
          view: 'pending',
          kind: 'sample-kind',
          tag: 'sample-tag',
          tab: 'annotations',
        },
      },
      {
        pathname: '/notes',
        search: { view: 'pending', kind: 'sample-kind', tag: 'sample-tag' },
      },
    ])
  })
})
