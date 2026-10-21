import { TabsPanel } from '@fohte/ui/tabs'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  type StrategyListTab,
  StrategyListTabs,
  validateStrategyListSearch,
} from '#components/strategy-home/strategy-list-tabs'

afterEach(cleanup)

function TabsHarness({ initialTab }: { initialTab: StrategyListTab }) {
  const [activeTab, setActiveTab] = useState(initialTab)

  return (
    <StrategyListTabs activeTab={activeTab} onChange={setActiveTab}>
      <TabsPanel value="list">
        {activeTab === 'list' && '架空の戦略一覧'}
      </TabsPanel>
      <TabsPanel value="runs">
        {activeTab === 'runs' && '架空の実行履歴'}
      </TabsPanel>
    </StrategyListTabs>
  )
}

function getTabsOutput() {
  return {
    tabs: screen.getAllByRole('tab').map((tab) => ({
      label: tab.textContent,
      selected: tab.getAttribute('aria-selected') === 'true',
    })),
    panel: screen.getByRole('tabpanel').textContent,
  }
}

function getTabTransitionOutput(
  initial: ReturnType<typeof getTabsOutput>,
  afterChange: ReturnType<typeof getTabsOutput>,
) {
  return { initial, afterChange }
}

describe('StrategyListTabs', () => {
  it.each([
    {
      initialTab: 'list',
      nextTab: 'runs',
      expected: {
        initial: {
          tabs: [
            { label: '一覧', selected: true },
            { label: '実行履歴', selected: false },
          ],
          panel: '架空の戦略一覧',
        },
        afterChange: {
          tabs: [
            { label: '一覧', selected: false },
            { label: '実行履歴', selected: true },
          ],
          panel: '架空の実行履歴',
        },
      },
    },
    {
      initialTab: 'runs',
      nextTab: 'list',
      expected: {
        initial: {
          tabs: [
            { label: '一覧', selected: false },
            { label: '実行履歴', selected: true },
          ],
          panel: '架空の実行履歴',
        },
        afterChange: {
          tabs: [
            { label: '一覧', selected: true },
            { label: '実行履歴', selected: false },
          ],
          panel: '架空の戦略一覧',
        },
      },
    },
  ] as const)(
    'タブを切り替えると対応する内容を表示する: $initialTab',
    async ({ initialTab, nextTab, expected }) => {
      const user = userEvent.setup()
      render(<TabsHarness initialTab={initialTab} />)
      const initial = getTabsOutput()

      await user.click(
        screen.getByRole('tab', {
          name: nextTab === 'list' ? '一覧' : '実行履歴',
        }),
      )

      expect(getTabTransitionOutput(initial, getTabsOutput())).toEqual(expected)
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
