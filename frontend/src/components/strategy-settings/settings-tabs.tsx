import { Link } from '@tanstack/react-router'

export const STRATEGY_SETTINGS_TABS = [
  { key: 'general', label: '全般' },
  { key: 'triggers', label: 'トリガー' },
  { key: 'indicators', label: '自作指標' },
] as const

export type StrategySettingsTab = (typeof STRATEGY_SETTINGS_TABS)[number]['key']

export function parseStrategySettingsTab(
  value: unknown,
): StrategySettingsTab | undefined {
  return STRATEGY_SETTINGS_TABS.find((tab) => tab.key === value)?.key
}

export function validateStrategySettingsSearch(
  search: Record<string, unknown>,
): { settingsTab?: StrategySettingsTab } {
  const settingsTab = parseStrategySettingsTab(search.settingsTab)
  return settingsTab ? { settingsTab } : {}
}

export function StrategySettingsTabs({
  strategyId,
  activeTab,
}: {
  strategyId: string
  activeTab: StrategySettingsTab
}) {
  return (
    <div
      role="tablist"
      aria-label="戦略設定タブ"
      className="flex items-center gap-1 border-b border-border"
    >
      {STRATEGY_SETTINGS_TABS.map((tab) => {
        const active = activeTab === tab.key
        return (
          <Link
            key={tab.key}
            to="/strategies/$id/settings"
            params={{ id: strategyId }}
            search={{ settingsTab: tab.key }}
            id={`strategy-settings-tab-${tab.key}`}
            role="tab"
            aria-selected={active}
            aria-controls="strategy-settings-panel"
            className="border-b-2 px-3 py-1.5 font-mono text-xs uppercase tracking-wider data-[active=true]:border-primary data-[active=true]:text-primary data-[active=false]:border-transparent data-[active=false]:text-muted-foreground"
            data-active={active}
          >
            {tab.label}
          </Link>
        )
      })}
    </div>
  )
}
