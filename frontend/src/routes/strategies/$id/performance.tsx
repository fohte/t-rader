import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useMemo } from 'react'

import { PositionsTable } from '#components/portfolio/positions-table'
import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import { formatYen, pnlColorClass } from '#components/trades/format'
import { TradesTable } from '#components/trades/trades-table'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/$id/performance')({
  component: StrategyPerformancePage,
})

function StrategyPerformancePage() {
  const { id } = Route.useParams()
  const navigate = useNavigate()

  const { data: strategy, isPending: strategyPending } = $api.useQuery(
    'get',
    '/api/strategies/{id}',
    { params: { path: { id } } },
  )
  const { data: summary, isPending: summaryPending } = $api.useQuery(
    'get',
    '/api/trades/summary',
    { params: { query: { strategy_id: id } } },
  )
  const { data: trades = [] } = $api.useQuery('get', '/api/trades', {
    params: { query: { strategy_id: id } },
  })
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')

  const sellsCount = trades.filter((t) => t.side === 'sell').length
  const feesTotal = trades.reduce((s, t) => s + t.fee, 0)

  const openPositions = useMemo(
    () => (summary?.positions ?? []).filter((p) => p.qty > 0),
    [summary],
  )
  const realizedPnl = summary?.realized_pnl ?? 0

  const goToTrades = () => {
    void navigate({ to: '/trades' })
  }

  const stats: StatItem[] = [
    {
      label: '実現損益',
      value: formatYen(realizedPnl, true),
      cls: pnlColorClass(realizedPnl),
    },
    {
      label: '手数料',
      value: formatYen(feesTotal),
      cls: 'text-muted-foreground-strong',
    },
    { label: '決済回数', value: sellsCount.toLocaleString() },
    { label: 'トレード件数', value: trades.length.toLocaleString() },
    { label: '保有銘柄', value: openPositions.length.toLocaleString() },
  ]

  if (strategyPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-72" />
        <Skeleton className="h-5 w-full max-w-[480px]" />
        <Skeleton className="h-[200px] w-full" />
      </div>
    )
  }

  if (strategy == null) {
    return (
      <div className="font-mono text-[13px] text-muted-foreground">
        戦略が見つかりませんでした。
      </div>
    )
  }

  return (
    <div className="space-y-5 font-sans text-foreground">
      <div>
        <Link
          to="/strategies/$id"
          params={{ id }}
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; {strategy.name} に戻る
        </Link>
      </div>

      <header>
        <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
          戦略成績 — {strategy.name}
        </h1>
        {strategy.description != null && strategy.description !== '' && (
          <p className="max-w-[720px] text-sm leading-relaxed text-muted-foreground-strong">
            {strategy.description}
          </p>
        )}
      </header>

      {summaryPending ? (
        <Skeleton className="h-[88px] w-full" />
      ) : (
        <StatRow stats={stats} />
      )}

      <section className="space-y-2">
        <div className="flex items-baseline justify-between">
          <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="mr-2 text-primary">&gt;</span>
            保有ポジション
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {openPositions.length} 銘柄
          </span>
        </div>
        {summaryPending ? (
          <Skeleton className="h-[160px] w-full" />
        ) : (
          <PositionsTable positions={openPositions} stocks={stocks} />
        )}
      </section>

      <section className="space-y-2">
        <div className="flex items-baseline justify-between">
          <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="mr-2 text-primary">&gt;</span>
            トレード
          </h2>
          <span className="font-mono text-2xs text-muted-foreground">
            {trades.length} 件
          </span>
        </div>
        <TradesTable
          trades={trades}
          strategies={strategies}
          stocks={stocks}
          showStrategy={false}
          onEdit={goToTrades}
          onDelete={goToTrades}
        />
      </section>

      <div>
        <Link
          to="/trades"
          className="font-mono text-[13px] text-primary hover:underline"
        >
          取引履歴で編集する →
        </Link>
      </div>
    </div>
  )
}
