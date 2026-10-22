import { Tabs, TabsList, TabsTab } from '@fohte/ui/tabs'
import { Link } from '@tanstack/react-router'

const STRATEGY_SETTINGS_TABS = [
  { key: 'general', label: '全般' },
  { key: 'triggers', label: 'トリガー' },
  { key: 'indicators', label: '自作指標' },
] as const

export type StrategySettingsTab = (typeof STRATEGY_SETTINGS_TABS)[number]['key']

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function parseStrategySettingsTab(
  value: unknown,
): StrategySettingsTab | undefined {
  return STRATEGY_SETTINGS_TABS.find((tab) => tab.key === value)?.key
}

export function validateStrategySettingsSearch(search: unknown): {
  settingsTab?: StrategySettingsTab
} {
  const settingsTab = parseStrategySettingsTab(
    isRecord(search) ? search.settingsTab : undefined,
  )
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
    <Tabs value={activeTab}>
      <TabsList aria-label="戦略設定タブ">
        {STRATEGY_SETTINGS_TABS.map((tab) => (
          <TabsTab
            key={tab.key}
            value={tab.key}
            nativeButton={false}
            render={
              <Link
                to="/strategies/$id/settings"
                params={{ id: strategyId }}
                search={{ settingsTab: tab.key }}
                id={`strategy-settings-tab-${tab.key}`}
                aria-controls="strategy-settings-panel"
              />
            }
          >
            {tab.label}
          </TabsTab>
        ))}
      </TabsList>
    </Tabs>
  )
}
