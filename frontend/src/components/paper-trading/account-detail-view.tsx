import { Link } from '@tanstack/react-router'

import { formatPercent } from '#components/paper-trading/format'
import { PaperTradeOrdersTable } from '#components/paper-trading/orders-table'
import { PaperTradePositionsTable } from '#components/paper-trading/positions-table'
import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PaperAccountStats = components['schemas']['PaperAccountStats']
type PaperAccountPortfolio = components['schemas']['PaperAccountPortfolio']

export function PaperTradingAccountDetailView({
  stats,
  portfolio,
  strategyName,
  stockNames,
}: {
  stats: PaperAccountStats
  portfolio: PaperAccountPortfolio
  strategyName: string
  stockNames: Record<string, string>
}) {
  const benchmarkReturn = stats.benchmark_return
  const winRate =
    stats.closed_trade_count === 0 || stats.win_rate == null
      ? '—'
      : formatPercent(stats.win_rate)
  const averageWin =
    stats.average_win_excess_return == null
      ? '—'
      : formatPercent(stats.average_win_excess_return, true)
  const averageLoss =
    stats.average_loss_excess_return == null
      ? '—'
      : formatPercent(stats.average_loss_excess_return, true)

  const summaryStats: StatItem[] = [
    {
      label: '総資産',
      value: formatYen(stats.total_assets_jpy),
    },
    {
      label: '現金',
      value: formatYen(portfolio.cash_jpy),
    },
    {
      label: '開始来 / ベンチマーク',
      value: (
        <>
          <span className={pnlColorClass(stats.return_since_start)}>
            {formatPercent(stats.return_since_start, true)}
          </span>
          <span className="text-muted-foreground-strong"> / </span>
          <span
            className={
              benchmarkReturn == null
                ? 'text-muted-foreground-strong'
                : pnlColorClass(benchmarkReturn)
            }
          >
            {benchmarkReturn == null
              ? '—'
              : formatPercent(benchmarkReturn, true)}
          </span>
        </>
      ),
    },
    {
      label: '勝率 (決済 ' + stats.closed_trade_count.toLocaleString() + ')',
      value: winRate,
    },
    {
      label: '勝ち平均超過',
      value: averageWin,
      cls:
        stats.average_win_excess_return == null
          ? undefined
          : pnlColorClass(stats.average_win_excess_return),
    },
    {
      label: '負け平均超過',
      value: averageLoss,
      cls:
        stats.average_loss_excess_return == null
          ? undefined
          : pnlColorClass(stats.average_loss_excess_return),
    },
    {
      label: '含み損益',
      value: formatYen(stats.unrealized_pnl_jpy, true),
      cls: pnlColorClass(stats.unrealized_pnl_jpy),
    },
  ]

  return (
    <div className="space-y-5 font-sans text-foreground">
      <div>
        <Link
          to="/paper-trading"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; ペーパートレード一覧に戻る
        </Link>
      </div>

      <header className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          {portfolio.account_name}
        </h1>
        <span className="border border-border bg-surface-strong px-1.5 py-0.5 font-mono text-2xs text-muted-foreground-strong">
          {strategyName}
        </span>
        <span className="border border-border bg-surface-strong px-1.5 py-0.5 font-mono text-2xs text-muted-foreground-strong">
          {portfolio.purpose}
        </span>
      </header>

      <StatRow stats={summaryStats} />

      <section className="space-y-2">
        <div className="flex items-baseline justify-between">
          <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="mr-2 text-primary">&gt;</span>
            保有ポジション
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {portfolio.positions.length.toLocaleString()} 銘柄
          </span>
        </div>
        <PaperTradePositionsTable
          positions={portfolio.positions}
          stockNames={stockNames}
        />
      </section>

      <section className="space-y-2">
        <div className="flex items-baseline justify-between">
          <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="mr-2 text-primary">&gt;</span>
            注文
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {portfolio.orders.length.toLocaleString()} 件
          </span>
        </div>
        <PaperTradeOrdersTable
          orders={portfolio.orders}
          stockNames={stockNames}
        />
      </section>
    </div>
  )
}
