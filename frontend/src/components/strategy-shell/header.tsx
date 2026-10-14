import { Link, useRouterState } from '@tanstack/react-router'
import { Menu as MenuIcon } from 'lucide-react'
import { type ReactNode, useState } from 'react'

import {
  NAV_PILL_ACTIVE,
  NAV_PILL_INACTIVE,
} from '#components/nav-pill-classes'
import { useReviewQueue } from '#components/review/use-review-queue'
import { StockSearch } from '#components/stocks/stock-search'
import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'
import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { cn } from '#lib/utils'

const MENU_LINK_INACTIVE =
  'flex items-center border border-border px-2.5 py-1.5 font-mono text-xs text-muted-foreground-strong hover:border-muted-foreground hover:bg-surface-strong hover:text-foreground'
const MENU_LINK_ACTIVE =
  'flex items-center border border-muted-foreground bg-surface-strong px-2.5 py-1.5 font-mono text-xs text-foreground'

const MENU_ITEMS = [
  { to: '/notes', label: 'ノート', matches: ['/notes', '/annotations'] },
  { to: '/strategies', label: '戦略', matches: ['/strategies'] },
  {
    to: '/paper-trading',
    label: 'ペーパートレード',
    matches: ['/paper-trading'],
  },
  { to: '/calendar', label: 'カレンダー', matches: ['/calendar'] },
  { to: '/settings', label: '全体の設定', matches: ['/settings'] },
] as const

function isPathWithin(pathname: string, path: string): boolean {
  return pathname === path || pathname.startsWith(`${path}/`)
}

function NavLink({
  to,
  label,
  active,
  children,
}: {
  to: string
  label?: string
  active: boolean
  children?: ReactNode
}) {
  return (
    <Link
      to={to}
      className={active ? NAV_PILL_ACTIVE : NAV_PILL_INACTIVE}
      aria-current={active ? 'page' : undefined}
    >
      {label}
      {children}
    </Link>
  )
}

function MenuLink({
  to,
  label,
  active,
  strategyId,
  onOpenChange,
}: {
  to: string
  label: string
  active: boolean
  strategyId: string | undefined
  onOpenChange: (open: boolean) => void
}) {
  const linkProps = {
    className: active ? MENU_LINK_ACTIVE : MENU_LINK_INACTIVE,
    'aria-current': active ? ('page' as const) : undefined,
    onClick: () => {
      onOpenChange(false)
    },
  }

  if (to === '/calendar') {
    return (
      <Link to="/calendar" search={{ strategy: strategyId }} {...linkProps}>
        {label}
      </Link>
    )
  }

  return (
    <Link to={to} {...linkProps}>
      {label}
    </Link>
  )
}

function HeaderMenu({
  open,
  onOpenChange,
  isActive,
  pathname,
  strategyId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  isActive: boolean
  pathname: string
  strategyId: string | undefined
}) {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger
        render={
          <button
            type="button"
            aria-label="メニュー"
            title="メニュー"
            aria-current={isActive ? 'page' : undefined}
            className={cn(
              isActive ? NAV_PILL_ACTIVE : NAV_PILL_INACTIVE,
              'h-7 w-7 justify-center px-0 md:h-auto md:w-auto md:px-2.5',
            )}
          >
            <MenuIcon aria-hidden="true" className="size-3.5" />
            <span className="hidden md:inline">メニュー</span>
          </button>
        }
      />
      <PopoverContent
        align="end"
        sideOffset={8}
        className="w-56 space-y-2 p-2"
        style={{ maxWidth: 'calc(100vw - 1.5rem)' }}
      >
        <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
          <span className="mr-1 text-primary">&gt;</span>
          ページ
        </h2>
        <nav aria-label="その他のページ" className="flex flex-col gap-1">
          {MENU_ITEMS.map((item) => (
            <MenuLink
              key={item.to}
              to={item.to}
              label={item.label}
              active={item.matches.some((path) => isPathWithin(pathname, path))}
              strategyId={strategyId}
              onOpenChange={onOpenChange}
            />
          ))}
        </nav>
      </PopoverContent>
    </Popover>
  )
}

export function Header({
  menuOpen: controlledMenuOpen,
}: {
  menuOpen?: boolean
} = {}) {
  const strategyId = useCurrentStrategyId()
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  })
  const { data: reviewQueue, isError, isPending } = useReviewQueue()
  const [uncontrolledMenuOpen, setUncontrolledMenuOpen] = useState(false)
  const menuOpen = controlledMenuOpen ?? uncontrolledMenuOpen
  const reviewCount = reviewQueue?.counts.total ?? 0
  const reviewBadge = isPending
    ? '…'
    : isError
      ? '—'
      : reviewCount > 0
        ? String(reviewCount)
        : null
  const isMenuActive = MENU_ITEMS.some((item) =>
    item.matches.some((path) => isPathWithin(pathname, path)),
  )

  function handleMenuOpenChange(open: boolean) {
    if (controlledMenuOpen == null) setUncontrolledMenuOpen(open)
  }

  return (
    <header className="sticky top-0 z-20 border-b border-border bg-background">
      <div className="flex items-center gap-2 px-3 py-2 md:gap-4 md:px-5 md:py-3">
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

        <nav
          aria-label="メインナビゲーション"
          className="flex flex-shrink-0 items-center gap-2"
        >
          <NavLink
            to="/portfolio"
            label="資産"
            active={
              isPathWithin(pathname, '/portfolio') ||
              isPathWithin(pathname, '/trades')
            }
          />
          <NavLink
            to="/review"
            label="レビュー"
            active={isPathWithin(pathname, '/review')}
          >
            {reviewBadge != null && (
              <span className="border border-primary px-1 text-2xs leading-4 tabular-nums text-primary">
                {reviewBadge}
              </span>
            )}
          </NavLink>
        </nav>

        <div className="ml-auto flex flex-shrink-0 items-center gap-2">
          <StockSearch />
          <HeaderMenu
            open={menuOpen}
            onOpenChange={handleMenuOpenChange}
            isActive={isMenuActive}
            pathname={pathname}
            strategyId={strategyId}
          />
        </div>
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
