import { Tabs, TabsList, TabsTab } from '@fohte/ui/tabs'
import type { ReactNode } from 'react'

const STRATEGY_LIST_TAB_IDS = ['list', 'runs'] as const

type StrategyListTab = (typeof STRATEGY_LIST_TAB_IDS)[number]
type LegacyStrategyListTab = Extract<StrategyListTab, 'runs'>

function parseStrategyListTab(value: unknown) {
  return STRATEGY_LIST_TAB_IDS.find((tab) => tab === value)
}

export function validateStrategyListSearch(search: Record<string, unknown>) {
  if (search.strategyTab === 'indicators') return { legacyIndicatorsTab: true }
  const strategyTab = parseStrategyListTab(search.strategyTab)
  return strategyTab ? { strategyTab } : {}
}

export function getLegacyStrategyListTabRedirectOptions(
  tab: LegacyStrategyListTab,
) {
  return { to: '/strategies' as const, search: { strategyTab: tab } }
}

const TAB_LABELS: Record<StrategyListTab, string> = {
  list: '一覧',
  runs: '実行履歴',
}
const TABS = STRATEGY_LIST_TAB_IDS.map((id) => ({
  id,
  label: TAB_LABELS[id],
}))

interface StrategyListTabsProps {
  activeTab: StrategyListTab
  onChange: (tab: StrategyListTab) => void
  children?: ReactNode
}

export function StrategyListTabs({
  activeTab,
  onChange,
  children,
}: StrategyListTabsProps) {
  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => {
        const nextTab = parseStrategyListTab(value)
        if (nextTab != null) onChange(nextTab)
      }}
    >
      <div className="mb-6 space-y-2">
        <div className="flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
          <span className="text-muted-foreground-strong">&gt;</span>
          <span>strategy</span>
        </div>
        <TabsList aria-label="戦略一覧のタブ">
          {TABS.map((tab) => (
            <TabsTab key={tab.id} value={tab.id}>
              {tab.label}
            </TabsTab>
          ))}
        </TabsList>
      </div>
      {children}
    </Tabs>
  )
}
