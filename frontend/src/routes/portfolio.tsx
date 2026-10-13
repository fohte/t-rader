import { useQueries } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { AssetTabs } from '#components/assets/asset-tabs'
import { getTokyoDate } from '#components/calendar/calendar-date'
import {
  AllocationBar,
  type AllocationSegment,
} from '#components/portfolio/allocation-bar'
import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'
import { PositionsTable } from '#components/portfolio/positions-table'
import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import { formatYen, pnlColorClass } from '#components/trades/format'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import type { PortfolioHistoryPeriod } from '#lib/portfolio-history'

export const Route = createFileRoute('/portfolio')({
  component: PortfolioPage,
})

function PortfolioPage() {
  const [historyPeriod, setHistoryPeriod] =
    useState<PortfolioHistoryPeriod>('month')
  const { data: summary, isPending: summaryPending } = $api.useQuery(
    'get',
    '/api/trades/summary',
  )
  const {
    data: trades,
    isPending: tradesPending,
    isError: tradesError,
  } = $api.useQuery('get', '/api/trades')
  const firstTradeDate = useMemo(
    () =>
      trades?.reduce<string | undefined>(
        (earliest, trade) =>
          earliest == null || trade.date < earliest ? trade.date : earliest,
        undefined,
      ),
    [trades],
  )
  const today = getTokyoDate()
  const {
    data: portfolioHistory = [],
    isPending: historyPending,
    isError: historyError,
  } = $api.useQuery(
    'get',
    '/api/portfolio/history',
    {
      params: {
        query: {
          from: firstTradeDate ?? today,
          to: today,
        },
      },
    },
    { enabled: firstTradeDate != null },
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

  const equity = openPositions.reduce((s, p) => s + p.cost_basis, 0)
  // 投資可能額 (investableTotal) は保有株式の簿価 (equity) を含む枠のため、
  // 単純合算すると投資済み分を二重計上する。未投資分だけを現金として扱う。
  const cash = investableTotal - equity
  const totalAssets = investableTotal
  const cashRatio = totalAssets > 0 ? (cash / totalAssets) * 100 : 0
  const investedRatio = totalAssets > 0 ? 100 - cashRatio : 0
  const realizedPnl = summary?.realized_pnl ?? 0
  const historyStatus = tradesPending
    ? 'loading'
    : tradesError
      ? 'error'
      : firstTradeDate == null
        ? 'empty'
        : historyPending
          ? 'loading'
          : historyError
            ? 'error'
            : 'ready'
  const historyErrorMessage = tradesError
    ? '取引履歴を読み込めませんでした'
    : historyError
      ? '資産推移を読み込めませんでした'
      : undefined

  const stockNameById = useMemo(
    () => new Map(stocks.map((s) => [s.id, s.name])),
    [stocks],
  )

  const allocation: AllocationSegment[] = useMemo(() => {
    const segs: AllocationSegment[] = [...openPositions]
      .sort((a, b) => b.cost_basis - a.cost_basis)
      .map((p) => ({
        key: `pos:${p.symbol}`,
        label: stockNameById.get(p.symbol) ?? p.symbol,
        value: p.cost_basis,
        kind: 'position' as const,
      }))
    segs.push({ key: 'cash', label: '現金', value: cash, kind: 'cash' })
    return segs
  }, [openPositions, cash, stockNameById])

  const stats: StatItem[] = [
    { label: '総資産', value: formatYen(totalAssets) },
    { label: '評価額 (株式・簿価)', value: formatYen(equity) },
    {
      label: '現金',
      value: formatYen(cash),
      sub: `${cashRatio.toFixed(1)}% 比率`,
    },
    {
      label: '実現損益 (累計)',
      value: formatYen(realizedPnl, true),
      cls: pnlColorClass(realizedPnl),
    },
    {
      label: '保有銘柄',
      value: openPositions.length.toLocaleString(),
    },
  ]

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
        <StatRow stats={stats} />
      )}

      <PortfolioHistoryChart
        data={portfolioHistory}
        period={historyPeriod}
        status={historyStatus}
        errorMessage={historyErrorMessage}
        onPeriodChange={setHistoryPeriod}
      />

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
            <PositionsTable positions={openPositions} stocks={stocks} />
          )}
        </section>

        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="mr-2 text-primary">&gt;</span>
              アロケーション
            </h2>
            <span className="font-mono text-2xs text-muted-foreground">
              現金 {cashRatio.toFixed(1)}% / 投資 {investedRatio.toFixed(1)}%
            </span>
          </div>
          <div className="border border-border bg-card p-4">
            <AllocationBar segments={allocation} />
          </div>
        </section>
      </div>

      <p className="font-mono text-2xs text-muted-foreground">
        # 現在値の取得は未対応のため、評価額・アロケーションは取得簿価
        (cost_basis) で計算しています。
      </p>
    </div>
  )
}
