import { Link } from '@tanstack/react-router'
import { Settings as SettingsIcon } from 'lucide-react'

import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'

const NAV_BASE =
  'flex flex-shrink-0 items-center gap-1.5 whitespace-nowrap border px-2.5 py-1 font-mono text-xs'
const NAV_INACTIVE = `${NAV_BASE} border-border text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground`
const NAV_ACTIVE = `${NAV_BASE} border-muted-foreground bg-surface-strong text-foreground`

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
    <header className="sticky top-0 z-20 border-b border-border bg-background">
      <div className="flex items-center gap-3 px-3 py-2 md:gap-4 md:px-5 md:py-3">
        <Link
          to="/portfolio"
          className="inline-flex flex-shrink-0 items-baseline gap-2 font-mono"
          title="ポートフォリオへ"
        >
          <span className="text-xl font-bold text-primary">&gt;</span>
          <span className="hidden text-lg font-medium tracking-tight text-foreground md:inline">
            t-rader
          </span>
        </Link>

        <div className="flex min-w-0 flex-1 items-center gap-2 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          <NavLink to="/portfolio" label="ポートフォリオ" />
          <NavLink to="/trades" label="取引履歴" />
          <NavLink to="/notes" label="ノート" />
          <NavLink to="/annotations" label="アノテーション" />
          <NavLink to="/hypotheses" label="仮説" />
          <NavLink to="/runs" label="実行履歴" />
          <NavLink to="/indicators" label="indicators" />
          <NavLink to="/strategies" label="戦略" />
        </div>

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
      </div>

      {strategyId != null && (
        <div className="flex items-center gap-2 border-t border-border bg-surface-strong px-3 py-1.5 md:gap-3 md:px-5">
          <StrategySwitcher />
          <div className="flex flex-shrink-0 items-center gap-2">
            <Link
              to="/strategies/$id/performance"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_INACTIVE}
              activeProps={{ className: NAV_ACTIVE }}
            >
              成績
            </Link>
            <Link
              to="/strategies/$id/settings"
              params={{ id: strategyId }}
              activeOptions={{ exact: false }}
              className={NAV_INACTIVE}
              activeProps={{ className: NAV_ACTIVE }}
            >
              設定
            </Link>
          </div>
        </div>
      )}
    </header>
  )
}
