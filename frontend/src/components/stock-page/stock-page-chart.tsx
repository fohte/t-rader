import { Link } from '@tanstack/react-router'

import { CandlestickChart } from '#components/candlestick-chart'
import { ChartIndicatorControl } from '#components/chart-indicator-control'
import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'
import { Skeleton } from '#components/ui/skeleton'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import {
  addChartIndicator,
  CHART_INDICATOR_PANE_HEIGHT,
  type ChartIndicatorConfig,
  getChartIndicatorPaneCount,
  removeChartIndicator,
  updateChartIndicatorDeviation,
  updateChartIndicatorMacdPeriod,
  updateChartIndicatorPeriod,
  useChartIndicators,
} from '#lib/chart-indicator-settings'
import type { ChartCurrency } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']
type Trade = components['schemas']['TradeListItem']
const priceFormatter = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

type SelectedDateStatus =
  | { kind: 'loading'; label: string }
  | { kind: 'error'; label: string }
  | { kind: 'missing'; label: string }
  | { kind: 'bar'; label: string; bar: Bar }

function getSelectedDateStatus(
  date: string | undefined,
  bar: Bar | null | undefined,
  isLoading: boolean,
  hasError: boolean,
): SelectedDateStatus | null {
  if (date == null) return null
  if (isLoading)
    return { kind: 'loading', label: `${date} の日足を読み込み中です` }
  if (hasError)
    return { kind: 'error', label: `${date} の日足データを取得できません` }
  if (bar == null)
    return { kind: 'missing', label: `${date} の日足データがありません` }

  return {
    kind: 'bar',
    label: `${date} 始 ${priceFormatter.format(bar.open)} 高 ${priceFormatter.format(bar.high)} 安 ${priceFormatter.format(bar.low)} 終 ${priceFormatter.format(bar.close)}`,
    bar,
  }
}

export function StockPageChart({
  stockId,
  bars,
  displayRangeStart,
  trades,
  annotations,
  selectedAnnotationId,
  onSelectAnnotation,
  timeframe,
  onTimeframeChange,
  currency,
  isLoading = false,
  hasError = false,
  selectedDate,
  selectedDateLoading = false,
  selectedDateError = false,
  selectedDateBar,
  chartIndicators: configuredIndicators,
}: {
  stockId: string
  bars: Bar[]
  displayRangeStart?: string
  trades: Trade[]
  annotations: ChartAnnotation[]
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  currency?: ChartCurrency
  isLoading?: boolean
  hasError?: boolean
  selectedDate?: string
  selectedDateLoading?: boolean
  selectedDateError?: boolean
  selectedDateBar?: Bar | null
  chartIndicators?: ChartIndicatorConfig[]
}) {
  const savedIndicators = useChartIndicators()
  const indicators = configuredIndicators ?? savedIndicators
  const lowerPaneCount = getChartIndicatorPaneCount(indicators)
  const chartHeightStyle = {
    height: `calc(var(--stock-chart-base-height) + ${(
      lowerPaneCount * CHART_INDICATOR_PANE_HEIGHT
    ).toString()}px)`,
  }
  const selectedDateStatus = getSelectedDateStatus(
    selectedDate,
    selectedDateBar,
    selectedDateLoading,
    selectedDateError,
  )

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
        <ChartIndicatorControl
          indicators={indicators}
          onAdd={addChartIndicator}
          onUpdatePeriod={updateChartIndicatorPeriod}
          onUpdateMacdPeriod={updateChartIndicatorMacdPeriod}
          onUpdateDeviation={updateChartIndicatorDeviation}
          onRemove={removeChartIndicator}
        />
        <Link
          to="/stocks/$stockId/chart"
          params={{ stockId }}
          aria-label="全画面チャートを開く"
          className="shrink-0 font-mono text-xs text-muted-foreground-strong underline decoration-border underline-offset-4 hover:text-foreground hover:decoration-primary"
        >
          全画面 ↗
        </Link>
      </div>
      {isLoading ? (
        <Skeleton
          className="[--stock-chart-base-height:220px] w-full sm:[--stock-chart-base-height:440px] lg:[--stock-chart-base-height:480px]"
          style={chartHeightStyle}
        />
      ) : hasError ? (
        <div
          className="flex items-center justify-center border border-border bg-card font-mono text-xs text-destructive [--stock-chart-base-height:220px] sm:[--stock-chart-base-height:440px] lg:[--stock-chart-base-height:480px]"
          style={chartHeightStyle}
        >
          チャートの取得に失敗しました
        </div>
      ) : (
        <CandlestickChart
          bars={bars}
          indicators={indicators}
          displayRangeStart={displayRangeStart}
          trades={trades}
          annotations={annotations}
          onSelectAnnotation={onSelectAnnotation}
          selectedAnnotationId={selectedAnnotationId}
          highlightedTimestamp={selectedDateBar?.timestamp}
          currency={currency}
          intraday={timeframe !== '1d' && timeframe !== '1w'}
          className="w-full min-w-0 [--stock-chart-base-height:220px] sm:[--stock-chart-base-height:440px] lg:[--stock-chart-base-height:480px]"
          style={chartHeightStyle}
        />
      )}
      {selectedDate != null && selectedDateStatus != null && (
        <div
          aria-label={selectedDateStatus.label}
          className="flex flex-wrap items-center gap-x-4 gap-y-1 border border-border bg-card px-3 py-2 font-mono text-xs"
          role="status"
        >
          <span className="text-muted-foreground">{selectedDate}</span>
          {selectedDateStatus.kind === 'loading' ? (
            <span className="text-muted-foreground">読み込み中</span>
          ) : selectedDateStatus.kind === 'error' ? (
            <span className="text-destructive">取得に失敗しました</span>
          ) : selectedDateStatus.kind === 'missing' ? (
            <span className="text-muted-foreground">
              この日の四本値はありません
            </span>
          ) : (
            <dl className="flex flex-wrap gap-x-3 gap-y-1">
              <div className="flex gap-1">
                <dt className="text-muted-foreground">始</dt>
                <dd>{priceFormatter.format(selectedDateStatus.bar.open)}</dd>
              </div>
              <div className="flex gap-1">
                <dt className="text-muted-foreground">高</dt>
                <dd>{priceFormatter.format(selectedDateStatus.bar.high)}</dd>
              </div>
              <div className="flex gap-1">
                <dt className="text-muted-foreground">安</dt>
                <dd>{priceFormatter.format(selectedDateStatus.bar.low)}</dd>
              </div>
              <div className="flex gap-1">
                <dt className="text-muted-foreground">終</dt>
                <dd>{priceFormatter.format(selectedDateStatus.bar.close)}</dd>
              </div>
            </dl>
          )}
        </div>
      )}
    </section>
  )
}
