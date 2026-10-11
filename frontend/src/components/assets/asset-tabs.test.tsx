import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { AssetTabs } from '#components/assets/asset-tabs'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getAssetTabsOutput(router: ReturnType<typeof createStoryRouter>) {
  return {
    initialPathname: router.state.location.pathname,
    tabs: screen.getAllByRole('link').map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
      current: link.getAttribute('aria-current'),
    })),
  }
}

function getAssetTabsNavigationOutput(
  router: ReturnType<typeof createStoryRouter>,
  initial: ReturnType<typeof getAssetTabsOutput>,
) {
  return {
    initial,
    destination: {
      pathname: router.state.location.pathname,
      search: router.state.location.search,
    },
  }
}

describe('AssetTabs', () => {
  it.each([
    {
      name: 'holdings tab navigates to trades',
      initialPath: '/portfolio',
      activeTab: 'holdings',
      destinationLabel: '取引',
      expected: {
        initial: {
          initialPathname: '/portfolio',
          tabs: [
            { label: '保有', href: '/portfolio', current: 'page' },
            { label: '取引', href: '/trades', current: null },
          ],
        },
        destination: { pathname: '/trades', search: {} },
      },
    },
    {
      name: 'trades tab navigates to holdings',
      initialPath: '/trades',
      activeTab: 'trades',
      destinationLabel: '保有',
      expected: {
        initial: {
          initialPathname: '/trades',
          tabs: [
            { label: '保有', href: '/portfolio', current: null },
            { label: '取引', href: '/trades', current: 'page' },
          ],
        },
        destination: { pathname: '/portfolio', search: {} },
      },
    },
  ] as const)(
    '$name',
    async ({ initialPath, activeTab, destinationLabel, expected }) => {
      const user = userEvent.setup()
      const router = createStoryRouter(
        () => <AssetTabs activeTab={activeTab} />,
        {
          paths: ['/portfolio', '/trades'],
          initialPath,
        },
      )
      render(<RouterProvider router={router} />)

      await screen.findByRole('link', { name: destinationLabel })
      const initial = getAssetTabsOutput(router)
      await user.click(screen.getByRole('link', { name: destinationLabel }))

      expect(getAssetTabsNavigationOutput(router, initial)).toEqual(expected)
    },
  )
})
