import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  parseStrategyListTab,
  StrategyListTabs,
} from '#components/strategy-home/strategy-list-tabs'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getTabsOutput(navigation: HTMLElement) {
  return within(navigation)
    .getAllByRole('link')
    .map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
      current: link.getAttribute('aria-current') === 'page',
    }))
}

describe('StrategyListTabs', () => {
  it.each([
    { activeTab: 'list' },
    { activeTab: 'runs' },
    { activeTab: 'indicators' },
  ] as const)('$activeTab タブの URL と表示を持つ', async ({ activeTab }) => {
    const router = createStoryRouter(
      () => <StrategyListTabs activeTab={activeTab} />,
      {
        paths: ['/strategies'],
        initialPath: `/strategies?tab=${activeTab}`,
      },
    )
    render(<RouterProvider router={router} />)

    const navigation = await screen.findByRole('navigation', {
      name: '戦略一覧のタブ',
    })

    expect(getTabsOutput(navigation)).toEqual([
      {
        label: '一覧',
        href: '/strategies?tab=list',
        current: activeTab === 'list',
      },
      {
        label: '実行履歴',
        href: '/strategies?tab=runs',
        current: activeTab === 'runs',
      },
      {
        label: 'indicators',
        href: '/strategies?tab=indicators',
        current: activeTab === 'indicators',
      },
    ])
  })

  it.each([
    { value: 'list', expected: 'list' },
    { value: 'runs', expected: 'runs' },
    { value: 'indicators', expected: 'indicators' },
    { value: 'unknown', expected: undefined },
  ] as const)('$value search 値を検証する', ({ value, expected }) => {
    expect(parseStrategyListTab(value)).toEqual(expected)
  })
})
