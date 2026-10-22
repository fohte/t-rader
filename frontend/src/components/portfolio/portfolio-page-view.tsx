import { AssetTabs } from '#components/assets/asset-tabs'
import {
  AllocationBar,
  type AllocationSegment,
} from '#components/portfolio/allocation-bar'
import { AllocationRatios } from '#components/portfolio/allocation-ratios'
import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'
import type { PortfolioValuation } from '#components/portfolio/portfolio-valuation'
import { PositionsTable } from '#components/portfolio/positions-table'
import { StatRow } from '#components/portfolio/stat-row'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import type {
  PortfolioHistoryPeriod,
  PortfolioHistoryPoint,
} from '#lib/portfolio-history'

type PositionSummary = components['schemas']['PositionSummary']
type Stock = components['schemas']['Stock']
type HistoryStatus = 'loading' | 'empty' | 'error' | 'ready'

interface PortfolioPageViewProps {
  summaryPending: boolean
  valuation: PortfolioValuation
  history: {
    data: PortfolioHistoryPoint[]
    period: PortfolioHistoryPeriod
    status: HistoryStatus
    errorMessage?: string
  }
  onHistoryPeriodChange: (period: PortfolioHistoryPeriod) => void
  positions: PositionSummary[]
  stocks: Stock[]
  allocation: AllocationSegment[]
  priceObservedAt: string | null
}

export function PortfolioPageView({
  summaryPending,
  valuation,
  history,
  onHistoryPeriodChange,
  positions,
  stocks,
  allocation,
  priceObservedAt,
}: PortfolioPageViewProps) {
  return (
    <main className="space-y-5 font-sans text-foreground">
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

      <PortfolioHistoryChart
        data={history.data}
        period={history.period}
        status={history.status}
        errorMessage={history.errorMessage}
        onPeriodChange={onHistoryPeriodChange}
      />

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-portfolio-layout)">
        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="mr-2 text-muted-foreground">&gt;</span>
              保有銘柄
            </h2>
            <span className="font-mono text-2xs text-muted-foreground">
              {positions.length} 銘柄
            </span>
          </div>
          {summaryPending ? (
            <Skeleton className="h-40 w-full" />
          ) : (
            <PositionsTable
              positions={positions}
              stocks={stocks}
              displayMarketValue
            />
          )}
        </section>

        <section className="space-y-2">
          <div className="flex items-baseline justify-between">
            <h2 className="font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              <span className="mr-2 text-muted-foreground">&gt;</span>
              アロケーション
            </h2>
            <AllocationRatios
              cashRatio={valuation.cashRatio}
              investedRatio={valuation.investedRatio}
            />
          </div>
          <div className="border border-border bg-card p-4">
            <AllocationBar
              segments={allocation}
              priceObservedAt={priceObservedAt}
            />
          </div>
        </section>
      </div>
    </main>
  )
}
