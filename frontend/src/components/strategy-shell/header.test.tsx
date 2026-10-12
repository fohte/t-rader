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

function renderHeader(initialPath = PERFORMANCE_PATH, unreadCount = 2) {
  const router = createStoryRouter(() => <Header />, {
    paths: [
      '/strategies',
      '/strategies/$id/performance',
      '/strategies/$id/runs',
      '/strategies/$id/indicators',
      '/strategies/$id/settings',
      '/note-versions/pending',
      '/notes',
      '/annotations/$annoId',
      '/paper-trading',
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
    <StrategySwitcherQueryDecorator unreadCount={unreadCount}>
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

function getMenuOutput(menuButton: HTMLElement, navigation: HTMLElement) {
  return {
    menuActive: menuButton.getAttribute('aria-current') === 'page',
    links: getNavigationOutput(navigation),
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
      await screen.findByRole('link', { name: /レビュー/ })

      expect(getNavigationOutput(navigation)).toEqual([
        { label: '資産', href: '/portfolio', active: true },
        {
          label: 'レビュー2',
          href: '/note-versions/pending',
          active: false,
        },
      ])
    },
  )

  it('レビュー画面で未読数付きのレビューを active にする', async () => {
    renderHeader('/note-versions/pending')
    const navigation = await screen.findByRole('navigation', {
      name: 'メインナビゲーション',
    })
    await screen.findByRole('link', { name: /レビュー/ })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: '資産', href: '/portfolio', active: false },
      {
        label: 'レビュー2',
        href: '/note-versions/pending',
        active: true,
      },
    ])
  })

  it('未読が 0 件ならレビューのバッジを表示しない', async () => {
    renderHeader('/portfolio', 0)
    const navigation = await screen.findByRole('navigation', {
      name: 'メインナビゲーション',
    })
    const menuButton = await screen.findByRole('button', { name: 'メニュー' })
    await userEvent.setup().click(menuButton)
    await screen.findByRole('navigation', { name: 'その他のページ' })

    expect(getNavigationOutput(navigation)).toEqual([
      { label: '資産', href: '/portfolio', active: true },
      {
        label: 'レビュー',
        href: '/note-versions/pending',
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
      { label: 'ノート', href: '/notes', active: false },
      { label: '戦略', href: '/strategies', active: false },
      { label: 'ペーパートレード', href: '/paper-trading', active: false },
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

  it('メニュー内の画面ではメニューボタンと該当項目を active にする', async () => {
    renderHeader('/settings/rss-feeds')
    const menuButton = await screen.findByRole('button', { name: 'メニュー' })
    const user = userEvent.setup()
    await user.click(menuButton)
    const navigation = await screen.findByRole('navigation', {
      name: 'その他のページ',
    })

    expect(getMenuOutput(menuButton, navigation)).toEqual({
      menuActive: true,
      links: [
        { label: 'ノート', href: '/notes', active: false },
        { label: '戦略', href: '/strategies', active: false },
        { label: 'ペーパートレード', href: '/paper-trading', active: false },
        { label: 'カレンダー', href: '/calendar', active: false },
        { label: '全体の設定', href: '/settings', active: true },
      ],
    })
  })

  it('アノテーション詳細ではメニューとノートを active にする', async () => {
    renderHeader('/annotations/demo-annotation')
    const menuButton = await screen.findByRole('button', { name: 'メニュー' })
    const user = userEvent.setup()
    await user.click(menuButton)
    const navigation = await screen.findByRole('navigation', {
      name: 'その他のページ',
    })

    expect(getMenuOutput(menuButton, navigation)).toEqual({
      menuActive: true,
      links: [
        { label: 'ノート', href: '/notes', active: true },
        { label: '戦略', href: '/strategies', active: false },
        { label: 'ペーパートレード', href: '/paper-trading', active: false },
        { label: 'カレンダー', href: '/calendar', active: false },
        { label: '全体の設定', href: '/settings', active: false },
      ],
    })
  })

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
        expect(
          getStrategyNavigationOutput(
            router.state.location.pathname,
            navigationElement,
          ),
        ).toEqual(getExpectedStrategyNavigation(path, activeTab))
      })
    },
  )
})
