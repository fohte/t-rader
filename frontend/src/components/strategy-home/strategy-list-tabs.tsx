import { Link } from '@tanstack/react-router'

const STRATEGY_LIST_TAB_IDS = ['list', 'runs'] as const

export type StrategyListTab = (typeof STRATEGY_LIST_TAB_IDS)[number]
export type LegacyStrategyListTab = Extract<StrategyListTab, 'runs'>

export function parseStrategyListTab(value: unknown) {
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

const TAB_BASE =
  'flex flex-shrink-0 items-center gap-1.5 whitespace-nowrap border px-2.5 py-1 font-mono text-xs'
const TAB_INACTIVE = `${TAB_BASE} border-border text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground`
const TAB_ACTIVE = `${TAB_BASE} border-muted-foreground bg-surface-strong text-foreground`

export function StrategyListTabs({
  activeTab,
}: {
  activeTab: StrategyListTab
}) {
  return (
    <div className="mb-6 space-y-2">
      <div className="flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
        <span className="text-primary">&gt;</span>
        <span>strategy</span>
      </div>
      <nav
        aria-label="戦略一覧のタブ"
        className="flex flex-wrap items-center gap-2 border-b border-border pb-3"
      >
        {TABS.map((tab) => (
          <Link
            key={tab.id}
            to="/strategies"
            search={{ strategyTab: tab.id }}
            activeOptions={{ exact: true }}
            className={activeTab === tab.id ? TAB_ACTIVE : TAB_INACTIVE}
            aria-current={activeTab === tab.id ? 'page' : undefined}
          >
            {tab.label}
          </Link>
        ))}
      </nav>
    </div>
  )
}
