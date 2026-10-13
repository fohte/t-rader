import { useQueries } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { AssetTabs } from '#components/assets/asset-tabs'
import {
  AllocationBar,
  type AllocationSegment,
} from '#components/portfolio/allocation-bar'
import { calculatePortfolioValuation } from '#components/portfolio/portfolio-valuation'
import { PositionsTable } from '#components/portfolio/positions-table'
import { StatRow } from '#components/portfolio/stat-row'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/portfolio')({
  component: PortfolioPage,
})

function PortfolioPage() {
  const { data: summary, isPending: summaryPending } = $api.useQuery(
    'get',
    '/api/trades/summary',
  )
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const investableAmountQueries = useQueries({
    queries: strategies.map((s) =>
      $api.queryOptions('get', '/api/strategies/{id}/investable-amount', {
        params: { path: { id: s.id } },
      }),
    ),
  })
  const investableTotal = investableAmountQueries.reduce(
    (sum, q) => sum + (q.data?.amount_jpy ?? 0),
    0,
  )

  const openPositions = useMemo(
    () => (summary?.positions ?? []).filter((p) => p.qty > 0),
    [summary],
  )

  const realizedPnl = summary?.realized_pnl ?? 0
  const valuation = calculatePortfolioValuation(
    openPositions,
    investableTotal,
    realizedPnl,
  )

  const stockNameById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )

  const allocation: AllocationSegment[] = useMemo(() => {
    const segs: AllocationSegment[] = [...openPositions]
      .sort((a, b) => {
        if (a.market_value == null) return b.market_value == null ? 0 : 1
        if (b.market_value == null) return -1
        return b.market_value - a.market_value
      })
      .map((p) => ({
        key: `pos:${p.symbol}`,
        label: stockNameById.get(p.symbol) ?? p.symbol,
        value: p.market_value ?? null,
        kind: 'position' as const,
      }))
    segs.push({
      key: 'cash',
      label: '現金',
      value: valuation.cash,
      kind: 'cash',
    })
    return segs
  }, [openPositions, stockNameById, valuation.cash])

  return (
    <div className="space-y-5 font-sans text-foreground">
      <AssetTabs activeTab="holdings" />

      <header>
        <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
          ポートフォリオ
        </h1>
        <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
          戦略横断の全体ビュー。現金比率・全保有・全体損益を把握します。LLM
          もこのコンテキストを参照します。
        </p>
      </header>

      {summaryPending ? (
        <Skeleton className="h-22 w-full" />
      ) : (
        <StatRow stats={valuation.stats} />
      )}

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-portfolio-layout)">
        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="mr-2 text-primary">&gt;</span>
              保有銘柄
            </h2>
            <span className="font-mono text-2xs text-muted-foreground">
              {openPositions.length} 銘柄
            </span>
          </div>
          {summaryPending ? (
            <Skeleton className="h-40 w-full" />
          ) : (
            <PositionsTable
              positions={openPositions}
              stocks={stocks}
              displayMarketValue
            />
          )}
        </section>

        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="mr-2 text-primary">&gt;</span>
              アロケーション
            </h2>
            <span className="font-mono text-2xs text-muted-foreground">
              現金{' '}
              {valuation.cashRatio == null
                ? '—'
                : `${valuation.cashRatio.toFixed(1)}%`}{' '}
              / 投資{' '}
              {valuation.investedRatio == null
                ? '—'
                : `${valuation.investedRatio.toFixed(1)}%`}
            </span>
          </div>
          <div className="border border-border bg-card p-4">
            <AllocationBar
              segments={allocation}
              priceObservedAt={summary?.price_observed_at ?? null}
            />
          </div>
        </section>
      </div>
    </div>
  )
}
