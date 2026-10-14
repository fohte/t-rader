import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  StrategyListTabs,
  validateStrategyListSearch,
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
  it.each([{ activeTab: 'list' }, { activeTab: 'runs' }] as const)(
    '$activeTab タブの URL と表示を持つ',
    async ({ activeTab }) => {
      const router = createStoryRouter(
        () => <StrategyListTabs activeTab={activeTab} />,
        {
          paths: ['/strategies'],
          initialPath: `/strategies?strategyTab=${activeTab}`,
        },
      )
      render(<RouterProvider router={router} />)

      const navigation = await screen.findByRole('navigation', {
        name: '戦略一覧のタブ',
      })

      expect(getTabsOutput(navigation)).toEqual([
        {
          label: '一覧',
          href: '/strategies?strategyTab=list',
          current: activeTab === 'list',
        },
        {
          label: '実行履歴',
          href: '/strategies?strategyTab=runs',
          current: activeTab === 'runs',
        },
      ])
    },
  )

  it.each([
    { search: { strategyTab: 'list' }, expected: { strategyTab: 'list' } },
    { search: { strategyTab: 'runs' }, expected: { strategyTab: 'runs' } },
    {
      search: { strategyTab: 'indicators' },
      expected: { legacyIndicatorsTab: true },
    },
    { search: { strategyTab: 'unknown' }, expected: {} },
  ] as const)('search $search を検証する', ({ search, expected }) => {
    expect(validateStrategyListSearch(search)).toEqual(expected)
  })
})
