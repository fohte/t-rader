import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { DetailBackLink } from '#components/detail-back-link'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getBackLinkOutput(pathname: string, linkText: string | null) {
  return { pathname, linkText }
}

function getExpectedBackLinkOutput(to: string, label: string) {
  return { pathname: to, linkText: `< ${label}` }
}

describe('DetailBackLink', () => {
  it.each([
    {
      to: '/notes',
      label: 'ノート一覧に戻る',
      initialPath: '/notes/sample-note-id',
    },
    {
      to: '/annotations',
      label: 'アノテーション一覧に戻る',
      initialPath: '/annotations/sample-annotation-id',
    },
  ] as const)(
    'リンク先の一覧へ移動する ($label)',
    async ({ to, label, initialPath }) => {
      const user = userEvent.setup()
      const router = createStoryRouter(
        () => <DetailBackLink to={to} label={label} />,
        {
          paths: [
            '/notes',
            '/annotations',
            '/notes/$noteId',
            '/annotations/$annoId',
          ],
          initialPath,
        },
      )
      render(<RouterProvider router={router} />)

      await user.click(await screen.findByRole('link', { name: `< ${label}` }))

      expect(
        getBackLinkOutput(
          router.state.location.pathname,
          screen.getByRole('link').textContent,
        ),
      ).toEqual(getExpectedBackLinkOutput(to, label))
    },
  )
})
