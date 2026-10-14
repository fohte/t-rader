import { Link } from '@tanstack/react-router'

import { CandlestickChart } from '#components/candlestick-chart'
import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'
import { Skeleton } from '#components/ui/skeleton'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import type { ChartCurrency } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

export function StockPageChart({
  stockId,
  bars,
  annotations,
  selectedAnnotationId,
  onSelectAnnotation,
  timeframe,
  onTimeframeChange,
  currency,
  isLoading = false,
  hasError = false,
}: {
  stockId: string
  bars: Bar[]
  annotations: ChartAnnotation[]
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  currency?: ChartCurrency
  isLoading?: boolean
  hasError?: boolean
}) {
  return (
    <section className="min-w-0 space-y-2" aria-label="銘柄チャート">
      <div className="flex items-center justify-between gap-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          <span className="mr-2 text-primary">&gt;</span>
          チャート
        </h2>
        <TimeframeSelector
          value={timeframe}
          onChange={onTimeframeChange}
          className="hidden sm:inline-flex"
        />
        <TimeframeSelector
          value={timeframe}
          onChange={onTimeframeChange}
          variant="compact"
          className="sm:hidden"
        />
        <Link
          to="/stocks/$stockId/chart"
          params={{ stockId }}
          aria-label="全画面チャートを開く"
          className="inline-flex h-6 shrink-0 items-center justify-center gap-1 rounded-(--btn-radius-xs) border border-border bg-background px-2 font-mono text-xs font-medium whitespace-nowrap transition-all outline-none select-none hover:bg-muted hover:text-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:translate-y-px"
        >
          全画面 ↗
        </Link>
      </div>
      {isLoading ? (
        <Skeleton className="h-55 w-full sm:h-110 lg:h-120" />
      ) : hasError ? (
        <div className="flex h-55 items-center justify-center border border-border bg-card font-mono text-xs text-destructive sm:h-110 lg:h-120">
          チャートの取得に失敗しました
        </div>
      ) : (
        <CandlestickChart
          bars={bars}
          annotations={annotations}
          onSelectAnnotation={onSelectAnnotation}
          selectedAnnotationId={selectedAnnotationId}
          currency={currency}
          intraday={timeframe !== '1d' && timeframe !== '1w'}
          className="h-55 w-full min-w-0 sm:h-110 lg:h-120"
        />
      )}
    </section>
  )
}
