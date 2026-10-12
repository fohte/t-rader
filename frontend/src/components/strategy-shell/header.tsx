import { Link } from '@tanstack/react-router'
import { Settings as SettingsIcon } from 'lucide-react'

import {
  NAV_PILL_ACTIVE,
  NAV_PILL_INACTIVE,
} from '#components/nav-pill-classes'
import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'

function NavLink({ to, label }: { to: string; label: string }) {
  return (
    <Link
      to={to}
      activeOptions={{ exact: false }}
      className={NAV_PILL_INACTIVE}
      activeProps={{ className: NAV_PILL_ACTIVE }}
    >
      {label}
    </Link>
  )
}

export function Header() {
  const strategyId = useCurrentStrategyId()

  return (
    <header className="sticky top-0 z-20 border-b border-border bg-background">
      <div className="flex items-center gap-3 px-3 py-2 md:gap-4 md:px-5 md:py-3">
        <Link
          to="/"
          className="inline-flex flex-shrink-0 items-baseline gap-2 font-mono"
          title="ダッシュボードへ"
          aria-label="ダッシュボードへ"
        >
          <span className="text-xl font-bold text-primary">&gt;</span>
          <span className="hidden text-lg font-medium tracking-tight text-foreground md:inline">
            t-rader
          </span>
        </Link>

        <div className="flex min-w-0 flex-1 items-center gap-2 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          <NavLink to="/portfolio" label="ポートフォリオ" />
          <NavLink to="/trades" label="取引履歴" />
          <NavLink to="/paper-trading" label="ペーパートレード" />
          <NavLink to="/notes" label="ノート" />
          <NavLink to="/annotations" label="アノテーション" />
          <NavLink to="/runs" label="実行履歴" />
          <NavLink to="/indicators" label="indicators" />
          <Link
            to="/calendar"
            search={{ strategy: strategyId }}
            activeOptions={{ exact: false }}
            className={NAV_PILL_INACTIVE}
            activeProps={{ className: NAV_PILL_ACTIVE }}
          >
            カレンダー
          </Link>
          <NavLink to="/strategies" label="戦略" />
        </div>

        <Link
          to="/settings"
          activeOptions={{ exact: false }}
          className={NAV_PILL_INACTIVE}
          activeProps={{ className: NAV_PILL_ACTIVE }}
          title="設定"
          aria-label="設定"
        >
          <SettingsIcon className="size-3.5" />
        </Link>
      </div>

      {strategyId != null && (
        <div className="flex items-center gap-2 border-t border-border bg-surface-strong px-3 py-1.5 md:gap-3 md:px-5">
          <StrategySwitcher />
          <nav
            aria-label="戦略ページ"
            className="flex flex-shrink-0 items-center gap-2"
          >
            <Link
              to="/strategies/$id/performance"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_PILL_INACTIVE}
              activeProps={{ className: NAV_PILL_ACTIVE }}
            >
              成績
            </Link>
            <Link
              to="/strategies/$id/runs"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_PILL_INACTIVE}
              activeProps={{ className: NAV_PILL_ACTIVE }}
            >
              実行履歴
            </Link>
            <Link
              to="/strategies/$id/indicators"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_PILL_INACTIVE}
              activeProps={{ className: NAV_PILL_ACTIVE }}
            >
              indicators
            </Link>
            <Link
              to="/strategies/$id/settings"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_PILL_INACTIVE}
              activeProps={{ className: NAV_PILL_ACTIVE }}
            >
              設定
            </Link>
          </nav>
        </div>
      )}
    </header>
  )
}
