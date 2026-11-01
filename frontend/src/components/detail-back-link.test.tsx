import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { DetailBackLink } from '#components/detail-back-link'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getBackLinkOutput(
  pathname: string,
  search: Record<string, unknown>,
  linkText: string | null,
) {
  return { pathname, search, linkText }
}

function getExpectedBackLinkOutput(
  tab: 'annotations' | undefined,
  label: string,
) {
  return {
    pathname: '/notes',
    search: tab == null ? {} : { tab },
    linkText: `< ${label}`,
  }
}

function getNotesReturnOutput(pathname: string, search: unknown) {
  return { pathname, search }
}

describe('DetailBackLink', () => {
  it.each([
    {
      tab: undefined,
      label: 'ノート一覧に戻る',
      initialPath: '/notes/sample-note-id',
    },
    {
      tab: 'annotations',
      label: 'アノテーション一覧に戻る',
      initialPath: '/annotations/sample-annotation-id',
    },
  ] as const)(
    'リンク先の一覧へ移動する ($label)',
    async ({ tab, label, initialPath }) => {
      const user = userEvent.setup()
      const router = createStoryRouter(
        () => <DetailBackLink tab={tab} label={label} />,
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
          router.state.location.search,
          screen.getByRole('link').textContent,
        ),
      ).toEqual(getExpectedBackLinkOutput(tab, label))
    },
  )

  it('restores the view and note filters from the overview detail', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(
      () => (
        <DetailBackLink
          label="ノート一覧に戻る"
          notesSearch={{
            view: 'pending',
            kind: 'sample-kind',
            tag: 'sample-tag',
          }}
        />
      ),
      {
        paths: ['/notes', '/notes/$noteId'],
        initialPath: '/notes/sample-note?version=3',
      },
    )
    render(<RouterProvider router={router} />)

    await user.click(
      await screen.findByRole('link', { name: '< ノート一覧に戻る' }),
    )

    expect(
      getNotesReturnOutput(
        router.state.location.pathname,
        router.state.location.search,
      ),
    ).toEqual({
      pathname: '/notes',
      search: { view: 'pending', kind: 'sample-kind', tag: 'sample-tag' },
    })
  })
})
