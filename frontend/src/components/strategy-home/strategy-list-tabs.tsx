import { Link } from '@tanstack/react-router'

export type StrategyListTab = 'list' | 'runs' | 'indicators'
export type LegacyStrategyListTab = Extract<
  StrategyListTab,
  'runs' | 'indicators'
>

export function getLegacyStrategyListTabRedirectOptions(
  tab: LegacyStrategyListTab,
) {
  return { to: '/strategies' as const, search: { tab } }
}

const TABS: { id: StrategyListTab; label: string }[] = [
  { id: 'list', label: '一覧' },
  { id: 'runs', label: '実行履歴' },
  { id: 'indicators', label: 'indicators' },
]

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
            search={{ tab: tab.id }}
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
