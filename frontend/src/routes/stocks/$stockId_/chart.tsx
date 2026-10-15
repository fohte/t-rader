import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import { type Timeframe } from '#components/timeframe-selector'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import { $api } from '#lib/api/client'
import {
  getChartIndicatorLookbackBars,
  useChartIndicators,
} from '#lib/chart-indicator-settings'
import { extendBarsRange, getBarsRange } from '#lib/chart-range'
import { getChartCurrency } from '#lib/chart-utils'

export const Route = createFileRoute('/stocks/$stockId_/chart')({
  component: StockChartPage,
})

const emptyAnnotations: ChartAnnotation[] = []

function StockChartPage() {
  const { stockId } = Route.useParams()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')
  const [isMarketDepthOpen, setIsMarketDepthOpen] = useState(false)
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<
    string | null
  >(null)
  const indicators = useChartIndicators()
  const lookbackBars = getChartIndicatorLookbackBars(indicators)
  const displayRange = useMemo(() => getBarsRange(timeframe), [timeframe])
  const chartRange = useMemo(
    () => extendBarsRange(displayRange, timeframe, lookbackBars),
    [displayRange, lookbackBars, timeframe],
  )

  const { data, isLoading, error } = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: stockId,
        timeframe,
        ...chartRange,
      },
    },
  })
  const { data: stock } = $api.useQuery('get', '/api/refs/stocks/{id}', {
    params: { path: { id: stockId } },
  })
  const { data: annotations, isPending: isAnnotationsPending } = $api.useQuery(
    'get',
    '/api/annotations',
    {
      params: { query: { target_symbol: stockId } },
    },
  )

  return (
    <StockChartPageView
      stockId={stockId}
      stockName={stock?.name ?? stockId}
      bars={data ?? []}
      displayRangeStart={displayRange.from}
      lookbackBars={lookbackBars}
      annotations={annotations ?? emptyAnnotations}
      timeframe={timeframe}
      onTimeframeChange={setTimeframe}
      isMarketDepthOpen={isMarketDepthOpen}
      onToggleMarketDepth={() => {
        setIsMarketDepthOpen((previous) => !previous)
      }}
      selectedAnnotationId={selectedAnnotationId}
      onSelectAnnotation={setSelectedAnnotationId}
      currency={getChartCurrency(stock?.market)}
      isLoading={isLoading}
      hasError={error != null}
      isAnnotationsLoading={isAnnotationsPending}
    />
  )
}
