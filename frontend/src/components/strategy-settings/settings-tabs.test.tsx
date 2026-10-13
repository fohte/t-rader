import { RouterProvider, useRouterState } from '@tanstack/react-router'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import {
  parseStrategySettingsTab,
  StrategySettingsTabs,
  validateStrategySettingsSearch,
} from '#components/strategy-settings/settings-tabs'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function RouterSettingsTabs() {
  const activeTab = useRouterState({
    select: (state) =>
      parseStrategySettingsTab(state.location.search.settingsTab) ?? 'general',
  })
  return (
    <StrategySettingsTabs strategyId="demo-strategy" activeTab={activeTab} />
  )
}

function getTabsOutput(navigation: HTMLElement) {
  return within(navigation)
    .getAllByRole('tab')
    .map((tab) => ({
      label: tab.textContent,
      href: tab.getAttribute('href'),
      selected: tab.getAttribute('aria-selected') === 'true',
    }))
}

describe('StrategySettingsTabs', () => {
  it.each([
    { activeTab: 'general' },
    { activeTab: 'triggers' },
    { activeTab: 'indicators' },
  ] as const)(
    '$activeTab タブの URL と選択状態を持つ',
    async ({ activeTab }) => {
      const router = createStoryRouter(() => <RouterSettingsTabs />, {
        paths: ['/strategies/$id/settings'],
        initialPath: `/strategies/demo-strategy/settings?settingsTab=${activeTab}`,
      })
      render(<RouterProvider router={router} />)

      const navigation = await screen.findByRole('tablist', {
        name: '戦略設定タブ',
      })

      expect(getTabsOutput(navigation)).toEqual([
        {
          label: '全般',
          href: '/strategies/demo-strategy/settings?settingsTab=general',
          selected: activeTab === 'general',
        },
        {
          label: 'トリガー',
          href: '/strategies/demo-strategy/settings?settingsTab=triggers',
          selected: activeTab === 'triggers',
        },
        {
          label: '自作指標',
          href: '/strategies/demo-strategy/settings?settingsTab=indicators',
          selected: activeTab === 'indicators',
        },
      ])
    },
  )

  it('タブを選ぶと search param を更新する', async () => {
    const user = userEvent.setup()
    const router = createStoryRouter(() => <RouterSettingsTabs />, {
      paths: ['/strategies/$id/settings'],
      initialPath: '/strategies/demo-strategy/settings?settingsTab=general',
    })
    render(<RouterProvider router={router} />)

    await user.click(await screen.findByRole('tab', { name: '自作指標' }))

    await waitFor(() => {
      expect(router.state.location.search).toEqual({
        settingsTab: 'indicators',
      })
    })

    const navigation = screen.getByRole('tablist', { name: '戦略設定タブ' })
    expect(getTabsOutput(navigation)).toEqual([
      {
        label: '全般',
        href: '/strategies/demo-strategy/settings?settingsTab=general',
        selected: false,
      },
      {
        label: 'トリガー',
        href: '/strategies/demo-strategy/settings?settingsTab=triggers',
        selected: false,
      },
      {
        label: '自作指標',
        href: '/strategies/demo-strategy/settings?settingsTab=indicators',
        selected: true,
      },
    ])
  })

  it.each([
    { value: 'general', expected: 'general' },
    { value: 'triggers', expected: 'triggers' },
    { value: 'indicators', expected: 'indicators' },
    { value: 'unknown', expected: undefined },
  ] as const)('$value search 値をタブへ変換する', ({ value, expected }) => {
    expect(parseStrategySettingsTab(value)).toEqual(expected)
  })

  it.each([
    {
      search: { settingsTab: 'general' },
      expected: { settingsTab: 'general' },
    },
    {
      search: { settingsTab: 'triggers' },
      expected: { settingsTab: 'triggers' },
    },
    {
      search: { settingsTab: 'indicators' },
      expected: { settingsTab: 'indicators' },
    },
    { search: { settingsTab: 'unknown' }, expected: {} },
  ] as const)('search $search をタブへ変換する', ({ search, expected }) => {
    expect(validateStrategySettingsSearch(search)).toEqual(expected)
  })
})
