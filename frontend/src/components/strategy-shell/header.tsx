import { Link } from '@tanstack/react-router'
import { Settings as SettingsIcon } from 'lucide-react'

import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'

const NAV_BASE =
  'flex flex-shrink-0 items-center gap-1.5 whitespace-nowrap border px-2.5 py-1 font-mono text-xs'
const NAV_INACTIVE = `${NAV_BASE} border-border-strategy text-text-secondary hover:border-text-tertiary hover:text-text-primary`
const NAV_ACTIVE = `${NAV_BASE} border-text-tertiary bg-panel-inset text-text-primary`

function NavLink({ to, label }: { to: string; label: string }) {
  return (
    <Link
      to={to}
      activeOptions={{ exact: false }}
      className={NAV_INACTIVE}
      activeProps={{ className: NAV_ACTIVE }}
    >
      {label}
    </Link>
  )
}

export function Header() {
  const strategyId = useCurrentStrategyId()

  return (
    <header className="sticky top-0 z-[25] flex items-center gap-3 border-b border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] px-3 py-2 md:gap-4 md:px-5 md:py-3">
      <Link
        to="/strategies"
        className="inline-flex flex-shrink-0 items-baseline gap-2 font-mono"
        title="戦略一覧へ"
      >
        <span className="text-[19px] font-bold text-[color:var(--color-accent-strategy)]">
          &gt;
        </span>
        <span className="hidden text-[17px] font-medium tracking-tight text-[color:var(--color-text-primary)] md:inline">
          t-rader
        </span>
      </Link>

      <StrategySwitcher />

      <div className="flex flex-shrink-0 items-center gap-2">
        <NavLink to="/portfolio" label="ポートフォリオ" />
        <NavLink to="/trades" label="取引履歴" />
        {strategyId != null && (
          <Link
            to="/strategies/$id/runs"
            params={{ id: strategyId }}
            activeOptions={{ exact: false }}
            className={NAV_INACTIVE}
            activeProps={{ className: NAV_ACTIVE }}
          >
            実行
          </Link>
        )}
        {strategyId != null ? (
          <Link
            to="/strategies/$id/indicators"
            params={{ id: strategyId }}
            activeOptions={{ exact: false }}
            className={NAV_INACTIVE}
            activeProps={{ className: NAV_ACTIVE }}
          >
            indicators
          </Link>
        ) : (
          <NavLink to="/indicators" label="indicators" />
        )}
        <Link
          to="/settings"
          activeOptions={{ exact: false }}
          className={NAV_INACTIVE}
          activeProps={{ className: NAV_ACTIVE }}
          title="設定"
          aria-label="設定"
        >
          <SettingsIcon className="size-3.5" />
        </Link>
        {strategyId != null && (
          <Link
            to="/strategies/$id/settings"
            params={{ id: strategyId }}
            className={NAV_INACTIVE}
            title="戦略設定"
            aria-label="戦略設定"
          >
            戦略設定
          </Link>
        )}
      </div>
    </header>
  )
}
