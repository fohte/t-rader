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

const STRATEGY_ID = 'demo-strategy'
const PERFORMANCE_PATH = `/strategies/${STRATEGY_ID}/performance`

function renderHeader(
  initialPath = PERFORMANCE_PATH,
  unreadCount = 2,
  reviewQueueError = false,
) {
  const router = createStoryRouter(() => <Header />, {
    paths: [
      '/strategies',
      '/strategies/$id/performance',
      '/strategies/$id/runs',
      '/strategies/$id/settings',
      '/notes',
      '/dashboards',
      '/dashboards/$key',
      '/annotations/$annoId',
      '/paper-trading',
      '/screener',
      '/calendar',
      '/settings',
      '/settings/rss-feeds',
      '/',
      '/portfolio',
      '/trades',
    ],
    initialPath,
  })
  render(
    <StrategySwitcherQueryDecorator
      unreadCount={unreadCount}
      reviewQueueError={reviewQueueError}
    >
      <RouterProvider router={router} />
    </StrategySwitcherQueryDecorator>,
  )
  return router
}

function getNavigationOutput(navigation: HTMLElement) {
  return within(navigation)
    .getAllByRole('link')
    .map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
      active: link.getAttribute('aria-current') === 'page',
    }))
}

function getQuickSwitcherTriggerOutput(button: HTMLElement) {
  return {
    label: button.getAttribute('aria-label'),
    expanded: button.getAttribute('aria-expanded'),
    hasSearchIcon: button.querySelector('svg')?.getAttribute('aria-hidden'),
  }
}

function getMenuOutput(menuButton: HTMLElement, navigation: HTMLElement) {
  return {
    menuActive: menuButton.getAttribute('aria-current') === 'page',
    links: getNavigationOutput(navigation),
  }
}

function getExpectedMenuOutput(
  activeLabel: string,
  calendarHref = '/calendar',
) {
  return {
    menuActive: true,
    links: [
      {
        label: 'ダッシュボード',
        href: '/dashboards',
        active: activeLabel === 'ダッシュボード',
      },
      { label: '戦略', href: '/strategies', active: activeLabel === '戦略' },
      {
        label: 'ペーパートレード',
        href: '/paper-trading',
        active: activeLabel === 'ペーパートレード',
      },
      {
        label: 'スクリーナー',
        href: '/screener',
        active: activeLabel === 'スクリーナー',
      },
      {
        label: 'カレンダー',
        href: calendarHref,
        active: activeLabel === 'カレンダー',
      },
      {
        label: '全体の設定',
        href: '/settings',
        active: activeLabel === '全体の設定',
      },
    ],
  }
}

function getMenuTransitionOutput(pathname: string, menuButton: HTMLElement) {
  return {
    pathname,
    menuOpen: menuButton.getAttribute('aria-expanded') === 'true',
  }
}

function getStrategyNavigationOutput(
  pathname: string,
  navigation: HTMLElement,
) {
  return {
    pathname,
    tabs: getNavigationOutput(navigation),
  }
}

function getStrategyBackLinkOutput(backLink: HTMLElement, pathname: string) {
  return {
    label: backLink.getAttribute('aria-label'),
    href: backLink.getAttribute('href'),
    text: backLink.textContent,
    pathname,
  }
}

function getExpectedStrategyNavigation(pathname: string, activeTab: string) {
  return {
    pathname,
    tabs: [
      {
        label: '成績',
        href: PERFORMANCE_PATH,
        active: false,
      },
      {
        label: '実行',
        href: `/strategies/${STRATEGY_ID}/runs`,
        active: activeTab === '実行',
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
  it('quick switcher のボタンを表示する', async () => {
    renderHeader()
    const button = await screen.findByRole('button', { name: '探す' })

    expect(getQuickSwitcherTriggerOutput(button)).toEqual({
      label: '探す',
      expanded: 'false',
      hasSearchIcon: 'true',
    })
  })

  it('ロゴからダッシュボードへ戻る', async () => {
    const user = userEvent.setup()
    const router = renderHeader()

    await user.click(
      await screen.findByRole('link', { name: 'ダッシュボードへ' }),
    )

    await waitFor(() => {
      expect(router.state.location.pathname).toEqual('/')
    })
  })

  it.each(['/portfolio', '/trades'])(
    '資産画面 %s で資産を active にする',
    async (path) => {
      renderHeader(path)
      const navigation = await screen.findByRole('navigation', {
        name: 'メインナビゲーション',
      })
      await screen.findByRole('link', { name: 'ノート2' })

      expect(getNavigationOutput(navigation)).toEqual([
        { label: '資産', href: '/portfolio', active: true },
        {
          label: 'ノート2',
          href: '/notes',
          active: false,
        },
      ])
    },
  )

  it('ノート画面で未読数付きの統一入口を active にする', async () => {
    renderHeader('/notes?view=pending')
    const navigation = await screen.findByRole('navigation', {
      name: 'メインナビゲーション',
    })
    await screen.findByRole('link', { name: 'ノート2' })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: '資産', href: '/portfolio', active: false },
      {
        label: 'ノート2',
        href: '/notes',
        active: true,
      },
    ])
  })

  it('未読が 0 件なら統一入口のバッジを表示しない', async () => {
    renderHeader('/portfolio', 0)
    const navigation = await screen.findByRole('navigation', {
      name: 'メインナビゲーション',
    })
    await screen.findByRole('link', { name: 'ノート' })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: '資産', href: '/portfolio', active: true },
      {
        label: 'ノート',
        href: '/notes',
        active: false,
      },
    ])
  })

  it('未読数の取得に失敗した場合はダッシュを表示する', async () => {
    renderHeader('/portfolio', 2, true)
    const navigation = await screen.findByRole('navigation', {
      name: 'メインナビゲーション',
    })
    await screen.findByRole('link', { name: 'ノート—' })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: '資産', href: '/portfolio', active: true },
      {
        label: 'ノート—',
        href: '/notes',
        active: false,
      },
    ])
  })

  it('メニューを開くと補助ページへのリンクを表示する', async () => {
    const user = userEvent.setup()
    renderHeader('/portfolio')

    await user.click(await screen.findByRole('button', { name: 'メニュー' }))
    const navigation = await screen.findByRole('navigation', {
      name: 'その他のページ',
    })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: 'ダッシュボード', href: '/dashboards', active: false },
      { label: '戦略', href: '/strategies', active: false },
      { label: 'ペーパートレード', href: '/paper-trading', active: false },
      { label: 'スクリーナー', href: '/screener', active: false },
      { label: 'カレンダー', href: '/calendar', active: false },
      { label: '全体の設定', href: '/settings', active: false },
    ])
  })

  it('メニューボタンを再度押すとメニューを閉じる', async () => {
    const user = userEvent.setup()
    renderHeader('/portfolio')
    const menuButton = await screen.findByRole('button', { name: 'メニュー' })

    await user.click(menuButton)
    await screen.findByRole('navigation', { name: 'その他のページ' })
    await user.click(menuButton)

    await waitFor(() => {
      expect(menuButton.getAttribute('aria-expanded')).toEqual('false')
    })
  })

  it('メニュー項目を選ぶと遷移してメニューを閉じる', async () => {
    const user = userEvent.setup()
    const router = renderHeader('/portfolio')
    const menuButton = await screen.findByRole('button', { name: 'メニュー' })

    await user.click(menuButton)
    const menuNavigation = await screen.findByRole('navigation', {
      name: 'その他のページ',
    })
    await user.click(
      within(menuNavigation).getByRole('link', { name: '全体の設定' }),
    )

    await waitFor(() => {
      expect(
        getMenuTransitionOutput(router.state.location.pathname, menuButton),
      ).toEqual({ pathname: '/settings', menuOpen: false })
    })
  })

  it.each([
    {
      pageName: 'ダッシュボード',
      path: '/dashboards/sample-market',
      activeLabel: 'ダッシュボード',
    },
    { pageName: '戦略一覧', path: '/strategies', activeLabel: '戦略' },
    {
      pageName: '戦略ページ',
      path: PERFORMANCE_PATH,
      activeLabel: '戦略',
      calendarHref: `/calendar?strategy=${STRATEGY_ID}`,
    },
    {
      pageName: 'ペーパートレード',
      path: '/paper-trading',
      activeLabel: 'ペーパートレード',
    },
    {
      pageName: 'スクリーナー',
      path: '/screener',
      activeLabel: 'スクリーナー',
    },
    { pageName: 'カレンダー', path: '/calendar', activeLabel: 'カレンダー' },
    {
      pageName: '全体の設定',
      path: '/settings/rss-feeds',
      activeLabel: '全体の設定',
    },
  ])(
    '$pageName ではメニューと該当項目を active にする',
    async ({ path, activeLabel, calendarHref }) => {
      renderHeader(path)
      const menuButton = await screen.findByRole('button', { name: 'メニュー' })
      const user = userEvent.setup()
      await user.click(menuButton)
      const navigation = await screen.findByRole('navigation', {
        name: 'その他のページ',
      })

      expect(getMenuOutput(menuButton, navigation)).toEqual(
        getExpectedMenuOutput(activeLabel, calendarHref),
      )
    },
  )

  it.each([
    {
      label: '実行',
      path: `/strategies/${STRATEGY_ID}/runs`,
      activeTab: '実行',
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
        expect(
          getStrategyNavigationOutput(
            router.state.location.pathname,
            navigationElement,
          ),
        ).toEqual(getExpectedStrategyNavigation(path, activeTab))
      })
    },
  )

  it('戦略ページから戦略一覧へ戻る', async () => {
    const user = userEvent.setup()
    const router = renderHeader()
    const backLink = await screen.findByRole('link', {
      name: '戦略一覧へ戻る',
    })

    await user.click(backLink)

    await waitFor(() => {
      expect(
        getStrategyBackLinkOutput(backLink, router.state.location.pathname),
      ).toEqual({
        label: '戦略一覧へ戻る',
        href: '/strategies',
        text: '←戦略一覧',
        pathname: '/strategies',
      })
    })
  })
})
