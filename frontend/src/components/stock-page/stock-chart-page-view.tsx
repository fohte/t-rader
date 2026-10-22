import { Button } from '@fohte/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@fohte/ui/tooltip'
import { Link } from '@tanstack/react-router'
import { Columns2Icon } from 'lucide-react'
import { type CSSProperties, useEffect } from 'react'

import { ChartAnnotationList } from '#components/annotations/chart-annotation-list'
import { CandlestickChart } from '#components/candlestick-chart'
import { ChartIndicatorControl } from '#components/chart-indicator-control'
import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'
import { HorizontalLineEditor } from '#components/stock-page/horizontal-line-editor'
import { TrendLineEditor } from '#components/stock-page/trend-line-editor'
import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'
import { Skeleton } from '#components/ui/skeleton'
import type {
  ChartAnnotation,
  HorizontalLineAnnotationDraft,
  HorizontalLinePoint,
  TrendLineAnnotationDraft,
  TrendLinePoint,
} from '#lib/annotation-chart-utils'
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

export function StockChartPageView({
  stockId,
  stockName,
  bars,
  displayRangeStart,
  annotations,
  timeframe,
  onTimeframeChange,
  isMarketDepthOpen,
  onToggleMarketDepth,
  selectedAnnotationId,
  onSelectAnnotation,
  horizontalLineDrawingEnabled,
  pendingHorizontalLine,
  isSavingHorizontalLine,
  horizontalLineSaveError,
  onToggleHorizontalLineDrawing,
  onPlaceHorizontalLine,
  onSaveHorizontalLine,
  onCancelHorizontalLine,
  trendLineDrawingEnabled,
  pendingTrendLineStart,
  pendingTrendLinePreviewPoint,
  pendingTrendLine,
  isSavingTrendLine,
  trendLineSaveError,
  onToggleTrendLineDrawing,
  onPlaceTrendLinePoint,
  onSaveTrendLine,
  onCancelTrendLine,
  isDeletingAnnotation,
  annotationDeleteError,
  onDeleteAnnotation,
  currency,
  isLoading,
  hasError,
  isAnnotationsLoading,
  chartIndicators: configuredIndicators,
}: {
  stockId: string
  stockName: string
  bars: Bar[]
  displayRangeStart?: string
  annotations: ChartAnnotation[]
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  isMarketDepthOpen: boolean
  onToggleMarketDepth: () => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  horizontalLineDrawingEnabled: boolean
  pendingHorizontalLine: HorizontalLinePoint | null
  isSavingHorizontalLine: boolean
  horizontalLineSaveError: string | null
  onToggleHorizontalLineDrawing: () => void
  onPlaceHorizontalLine: (line: HorizontalLinePoint) => void
  onSaveHorizontalLine: (draft: HorizontalLineAnnotationDraft) => void
  onCancelHorizontalLine: () => void
  trendLineDrawingEnabled: boolean
  pendingTrendLineStart: TrendLinePoint | null
  pendingTrendLinePreviewPoint?: TrendLinePoint | null
  pendingTrendLine: Pick<TrendLineAnnotationDraft, 'start' | 'end'> | null
  isSavingTrendLine: boolean
  trendLineSaveError: string | null
  onToggleTrendLineDrawing: () => void
  onPlaceTrendLinePoint: (point: TrendLinePoint) => void
  onSaveTrendLine: (draft: TrendLineAnnotationDraft) => void
  onCancelTrendLine: () => void
  isDeletingAnnotation: boolean
  annotationDeleteError: string | null
  onDeleteAnnotation: (id: string) => void
  currency?: ChartCurrency
  isLoading: boolean
  hasError: boolean
  isAnnotationsLoading: boolean
  chartIndicators?: ChartIndicatorConfig[]
}) {
  const savedIndicators = useChartIndicators()
  const indicators = configuredIndicators ?? savedIndicators
  const lowerPaneCount = getChartIndicatorPaneCount(indicators)
  const chartHeightStyle: CSSProperties & {
    '--stock-chart-height': string
  } = {
    '--stock-chart-height': `calc(600px + ${(lowerPaneCount * CHART_INDICATOR_PANE_HEIGHT).toString()}px)`,
  }
  const targetKinds = [
    ...new Set(annotations.map((annotation) => annotation.target_kind)),
  ]

  useEffect(() => {
    if (
      !horizontalLineDrawingEnabled &&
      pendingHorizontalLine == null &&
      !trendLineDrawingEnabled &&
      pendingTrendLineStart == null &&
      pendingTrendLine == null
    ) {
      return
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (
        event.key !== 'Escape' ||
        event.defaultPrevented ||
        isSavingHorizontalLine ||
        isSavingTrendLine
      ) {
        return
      }

      event.preventDefault()
      if (
        trendLineDrawingEnabled ||
        pendingTrendLineStart != null ||
        pendingTrendLine != null
      ) {
        onCancelTrendLine()
        return
      }

      onCancelHorizontalLine()
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [
    horizontalLineDrawingEnabled,
    isSavingHorizontalLine,
    isSavingTrendLine,
    onCancelHorizontalLine,
    onCancelTrendLine,
    pendingHorizontalLine,
    pendingTrendLine,
    pendingTrendLineStart,
    trendLineDrawingEnabled,
  ])
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
            onUpdateMacdPeriod={updateChartIndicatorMacdPeriod}
            onUpdateDeviation={updateChartIndicatorDeviation}
            onRemove={removeChartIndicator}
          />
          <Button
            variant={horizontalLineDrawingEnabled ? 'default' : 'outline'}
            size="sm"
            aria-label="水平線"
            aria-pressed={horizontalLineDrawingEnabled}
            disabled={pendingHorizontalLine != null || pendingTrendLine != null}
            title="水平線"
            onClick={() => {
              onToggleHorizontalLineDrawing()
            }}
          >
            <span aria-hidden="true">─</span>
            <span className="sr-only sm:not-sr-only sm:whitespace-nowrap">
              水平線
            </span>
          </Button>
          <Button
            variant={trendLineDrawingEnabled ? 'default' : 'outline'}
            size="sm"
            aria-label="トレンドライン"
            aria-pressed={trendLineDrawingEnabled}
            disabled={pendingHorizontalLine != null || pendingTrendLine != null}
            title="トレンドライン"
            onClick={() => {
              onToggleTrendLineDrawing()
            }}
          >
            <span aria-hidden="true">╱</span>
            <span className="sr-only sm:not-sr-only sm:whitespace-nowrap">
              トレンドライン
            </span>
          </Button>
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
      <div className="flex min-h-0 flex-1 flex-col gap-4 sm:flex-row">
        {isLoading ? (
          <Skeleton
            className="h-(--stock-chart-height) w-full"
            style={chartHeightStyle}
          />
        ) : hasError ? (
          <p
            className="flex h-(--stock-chart-height) flex-1 items-center text-destructive"
            style={chartHeightStyle}
          >
            データの取得に失敗しました
          </p>
        ) : (
          <>
            <CandlestickChart
              bars={bars}
              indicators={indicators}
              displayRangeStart={displayRangeStart}
              annotations={annotations}
              onSelectAnnotation={onSelectAnnotation}
              selectedAnnotationId={selectedAnnotationId}
              horizontalLineDrawingEnabled={horizontalLineDrawingEnabled}
              pendingHorizontalLine={pendingHorizontalLine}
              onPlaceHorizontalLine={onPlaceHorizontalLine}
              trendLineDrawingEnabled={trendLineDrawingEnabled}
              pendingTrendLineStart={pendingTrendLineStart}
              pendingTrendLinePreviewPoint={pendingTrendLinePreviewPoint}
              onPlaceTrendLinePoint={onPlaceTrendLinePoint}
              currency={currency}
              intraday={timeframe !== '1d' && timeframe !== '1w'}
              className="h-(--stock-chart-height) min-w-0 flex-1"
              style={chartHeightStyle}
            />
            <div
              className="flex h-(--stock-chart-height) min-h-0 w-full flex-col gap-3 sm:w-72 sm:shrink-0"
              style={chartHeightStyle}
            >
              {pendingHorizontalLine != null && (
                <HorizontalLineEditor
                  timestamp={pendingHorizontalLine.timestamp}
                  price={pendingHorizontalLine.price}
                  targetKinds={targetKinds}
                  isSaving={isSavingHorizontalLine}
                  errorMessage={horizontalLineSaveError}
                  onSave={onSaveHorizontalLine}
                  onCancel={onCancelHorizontalLine}
                />
              )}
              {pendingTrendLine != null && (
                <TrendLineEditor
                  start={pendingTrendLine.start}
                  end={pendingTrendLine.end}
                  targetKinds={targetKinds}
                  isSaving={isSavingTrendLine}
                  errorMessage={trendLineSaveError}
                  onSave={onSaveTrendLine}
                  onCancel={onCancelTrendLine}
                />
              )}
              <ChartAnnotationList
                annotations={annotations}
                selectedAnnotationId={selectedAnnotationId}
                isLoading={isAnnotationsLoading}
                isDeleting={isDeletingAnnotation}
                deleteError={annotationDeleteError}
                className="min-h-0 flex-1 sm:h-0"
                onSelectAnnotation={onSelectAnnotation}
                onDeleteAnnotation={onDeleteAnnotation}
              />
            </div>
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
