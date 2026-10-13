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
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          <span className="mr-2 text-primary">&gt;</span>
          チャート
        </h2>
        <TimeframeSelector value={timeframe} onChange={onTimeframeChange} />
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
