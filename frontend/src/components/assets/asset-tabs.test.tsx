import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { AssetTabs } from '#components/assets/asset-tabs'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getAssetTabsLocation(router: ReturnType<typeof createStoryRouter>) {
  return {
    pathname: router.state.location.pathname,
    search: router.state.location.search,
  }
}

describe('AssetTabs', () => {
  it('updates the URL when selecting a different asset tab', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(() => <AssetTabs activeTab="holdings" />, {
      paths: ['/portfolio', '/trades'],
      initialPath: '/portfolio',
    })
    render(<RouterProvider router={router} />)

    await user.click(await screen.findByRole('link', { name: '取引' }))

    expect(getAssetTabsLocation(router)).toEqual({
      pathname: '/trades',
      search: {},
    })
  })
})
