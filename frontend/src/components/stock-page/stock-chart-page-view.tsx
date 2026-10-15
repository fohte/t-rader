import { Button } from '@fohte/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@fohte/ui/tooltip'
import { Link } from '@tanstack/react-router'
import { Columns2Icon } from 'lucide-react'

import { ChartAnnotationList } from '#components/annotations/chart-annotation-list'
import { CandlestickChart } from '#components/candlestick-chart'
import { ChartIndicatorControl } from '#components/chart-indicator-control'
import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'
import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'
import { Skeleton } from '#components/ui/skeleton'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import {
  addChartIndicator,
  removeChartIndicator,
  updateChartIndicatorDeviation,
  updateChartIndicatorPeriod,
  useChartIndicators,
} from '#lib/chart-indicator-settings'
import type { ChartCurrency } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

export function StockChartPageView({
  stockId,
  stockName,
  bars,
  displayRangeStart,
  lookbackBars,
  annotations,
  timeframe,
  onTimeframeChange,
  isMarketDepthOpen,
  onToggleMarketDepth,
  selectedAnnotationId,
  onSelectAnnotation,
  currency,
  isLoading,
  hasError,
  isAnnotationsLoading,
}: {
  stockId: string
  stockName: string
  bars: Bar[]
  displayRangeStart?: string
  lookbackBars?: number
  annotations: ChartAnnotation[]
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  isMarketDepthOpen: boolean
  onToggleMarketDepth: () => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  currency?: ChartCurrency
  isLoading: boolean
  hasError: boolean
  isAnnotationsLoading: boolean
}) {
  const indicators = useChartIndicators()

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <h1 className="min-w-0 truncate text-2xl font-bold">
          <Link
            to="/stocks/$stockId"
            params={{ stockId }}
            className="hover:underline"
          >
            ← {stockName}
          </Link>
        </h1>
        <div className="flex shrink-0 items-center gap-2">
          <TimeframeSelector value={timeframe} onChange={onTimeframeChange} />
          <ChartIndicatorControl
            indicators={indicators}
            onAdd={addChartIndicator}
            onUpdatePeriod={updateChartIndicatorPeriod}
            onUpdateDeviation={updateChartIndicatorDeviation}
            onRemove={removeChartIndicator}
          />
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    variant={isMarketDepthOpen ? 'default' : 'outline'}
                    size="icon-sm"
                    onClick={onToggleMarketDepth}
                    aria-label="板情報・歩み値パネルの表示切替"
                    aria-pressed={isMarketDepthOpen}
                  >
                    <Columns2Icon />
                  </Button>
                }
              />
              <TooltipContent>板情報・歩み値</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 gap-4">
        {isLoading ? (
          <Skeleton className="h-150 w-full" />
        ) : hasError ? (
          <p className="text-destructive">データの取得に失敗しました</p>
        ) : (
          <>
            <CandlestickChart
              bars={bars}
              indicators={indicators}
              displayRangeStart={displayRangeStart}
              lookbackBars={lookbackBars}
              annotations={annotations}
              onSelectAnnotation={onSelectAnnotation}
              selectedAnnotationId={selectedAnnotationId}
              currency={currency}
              intraday={timeframe !== '1d' && timeframe !== '1w'}
              className="h-150 min-w-0 flex-1"
            />
            <ChartAnnotationList
              annotations={annotations}
              selectedAnnotationId={selectedAnnotationId}
              isLoading={isAnnotationsLoading}
              className="h-150"
              onSelectAnnotation={onSelectAnnotation}
            />
          </>
        )}
        <ChartMarketDepthPanel
          instrumentId={stockId}
          isOpen={isMarketDepthOpen}
          onToggle={onToggleMarketDepth}
        />
      </div>
    </div>
  )
}
