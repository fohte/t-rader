import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import {
  getScreenerIdFromFrontmatter,
  NoteScreenerLink,
} from '#components/note-detail/note-screener-link'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function readLocation(router: ReturnType<typeof createStoryRouter>) {
  return {
    pathname: router.state.location.pathname,
    search: router.state.location.search,
  }
}

describe('getScreenerIdFromFrontmatter', () => {
  it.each([
    ['screen id', { screen_id: 'fake-screen-a' }, 'fake-screen-a'],
    ['missing screen id', {}, undefined],
    ['empty screen id', { screen_id: '  ' }, undefined],
    ['non-object frontmatter', null, undefined],
  ] as const)(
    'extracts a screen id only from valid frontmatter',
    (_name, frontmatter, expected) => {
      expect(getScreenerIdFromFrontmatter(frontmatter)).toEqual(expected)
    },
  )
})

describe('NoteScreenerLink', () => {
  it('opens the saved screen selection in the screener', async () => {
    const user = userEvent.setup()
    const screenId = getScreenerIdFromFrontmatter({
      screen_id: 'fake-screen-a',
    })
    const router = createStoryRouter(
      () => <NoteScreenerLink screenId={screenId} />,
      {
        paths: ['/notes/$noteId', '/screener'],
        initialPath: '/notes/fake-note-a',
      },
    )
    render(<RouterProvider router={router} />)

    await user.click(
      await screen.findByRole('link', { name: 'スクリーナーで開く' }),
    )

    expect(readLocation(router)).toEqual({
      pathname: '/screener',
      search: { screen: 'fake-screen-a' },
    })
  })

  it('shows no link when the frontmatter has no screen id', () => {
    const router = createStoryRouter(
      () => (
        <NoteScreenerLink
          screenId={getScreenerIdFromFrontmatter({ tags: ['fake-tag'] })}
        />
      ),
      { paths: ['/screener'] },
    )
    render(<RouterProvider router={router} />)

    expect(screen.queryByRole('link')).toEqual(null)
  })
})
