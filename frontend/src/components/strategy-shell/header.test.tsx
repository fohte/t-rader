import { RouterProvider } from '@tanstack/react-router'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { Header } from '#components/strategy-shell/header'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

afterEach(cleanup)

const STRATEGY_ID = 'sample-alpha'
const PERFORMANCE_PATH = `/strategies/${STRATEGY_ID}/performance`

function renderHeader() {
  const router = createStoryRouter(() => <Header />, {
    paths: [
      '/strategies/$id/performance',
      '/strategies/$id/runs',
      '/strategies/$id/indicators',
      '/strategies/$id/settings',
    ],
    initialPath: PERFORMANCE_PATH,
  })
  render(
    <StrategySwitcherQueryDecorator>
      <RouterProvider router={router} />
    </StrategySwitcherQueryDecorator>,
  )
  return router
}

function getHeaderOutput(pathname: string, navigation: HTMLElement) {
  return {
    pathname,
    tabs: within(navigation)
      .getAllByRole('link')
      .map((link) => ({
        label: link.textContent,
        href: link.getAttribute('href'),
        active: link.getAttribute('aria-current') === 'page',
      })),
  }
}

function getExpectedHeaderOutput(pathname: string, activeTab: string) {
  return {
    pathname,
    tabs: [
      {
        label: '成績',
        href: PERFORMANCE_PATH,
        active: false,
      },
      {
        label: '実行履歴',
        href: `/strategies/${STRATEGY_ID}/runs`,
        active: activeTab === '実行履歴',
      },
      {
        label: 'indicators',
        href: `/strategies/${STRATEGY_ID}/indicators`,
        active: activeTab === 'indicators',
      },
      {
        label: '設定',
        href: `/strategies/${STRATEGY_ID}/settings`,
        active: false,
      },
    ],
  }
}

describe('Header', () => {
  it.each([
    {
      label: '実行履歴',
      path: `/strategies/${STRATEGY_ID}/runs`,
      activeTab: '実行履歴',
    },
    {
      label: 'indicators',
      path: `/strategies/${STRATEGY_ID}/indicators`,
      activeTab: 'indicators',
    },
  ])(
    '戦略の $label 画面へ移動して該当タブを active にする',
    async ({ label, path, activeTab }) => {
      const user = userEvent.setup()
      const router = renderHeader()
      const navigationElement = await screen.findByRole('navigation', {
        name: '戦略ページ',
      })
      const navigation = within(navigationElement)

      await user.click(navigation.getByRole('link', { name: label }))
      await waitFor(() => {
        expect(router.state.location.pathname).toBe(path)
      })

      expect(
        getHeaderOutput(router.state.location.pathname, navigationElement),
      ).toEqual(getExpectedHeaderOutput(path, activeTab))
    },
  )
})
