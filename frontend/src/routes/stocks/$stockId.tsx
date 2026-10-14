import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { StockPageState } from '#components/stock-page/stock-page-state'
import { StockPageView } from '#components/stock-page/stock-page-view'
import { type Timeframe } from '#components/timeframe-selector'
import { $api, fetchClient } from '#lib/api/client'
import { getBarsRange } from '#lib/chart-range'
import { getChartCurrency } from '#lib/chart-utils'
import {
  buildStockQuote,
  getStockPageListState,
  resolveStockLookup,
} from '#lib/stock-page-model'

export const Route = createFileRoute('/stocks/$stockId')({
  component: StockPage,
})

function StockPage() {
  const { stockId } = Route.useParams()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<
    string | null
  >(null)
  const [activeTab, setActiveTab] = useState<
    'annotations' | 'notes' | 'trades'
  >('annotations')
  const chartRange = useMemo(() => getBarsRange(timeframe), [timeframe])
  const dailyRange = useMemo(() => getBarsRange('1d'), [])

  const stockQuery = useQuery({
    queryKey: ['stock-page-stock', stockId],
    queryFn: async ({ signal }) => {
      const response = await fetchClient.GET('/api/refs/stocks/{id}', {
        params: { path: { id: stockId } },
        signal,
      })

      return resolveStockLookup({
        status: response.response.status,
        stock: response.data,
        error: response.error,
      })
    },
  })
  const chartBarsQuery = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: stockId,
        timeframe,
        ...chartRange,
      },
    },
  })
  const dailyBarsQuery = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: stockId,
        timeframe: '1d',
        ...dailyRange,
      },
    },
  })
  const annotationsQuery = $api.useQuery('get', '/api/annotations', {
    params: { query: { target_symbol: stockId } },
  })
  const tradesQuery = $api.useQuery('get', '/api/trades', {
    params: { query: { symbol: stockId } },
  })
  const notesQuery = $api.useQuery('get', '/api/notes', {
    params: { query: { ref: `stock:${stockId}` } },
  })
  const summaryQuery = $api.useQuery('get', '/api/trades/summary')

  if (stockQuery.isPending) return <StockPageState state="loading" />
  if (stockQuery.error != null) return <StockPageState state="error" />
  if (stockQuery.data.kind === 'not-found') {
    return <StockPageState state="not-found" />
  }
  if (stockQuery.data.kind === 'error') return <StockPageState state="error" />

  const stock = stockQuery.data.stock
  const position =
    summaryQuery.data?.positions.find(
      (item) => item.symbol === stockId && item.qty > 0,
    ) ?? null
  const quote = buildStockQuote(dailyBarsQuery.data ?? [])

  return (
    <StockPageView
      stock={stock}
      quote={quote}
      quoteLoading={dailyBarsQuery.isPending}
      quoteError={dailyBarsQuery.isError}
      bars={chartBarsQuery.data ?? []}
      annotations={annotationsQuery.data ?? []}
      notes={notesQuery.data ?? []}
      trades={tradesQuery.data ?? []}
      position={position}
      holdingsLoading={summaryQuery.isPending}
      holdingsError={summaryQuery.isError}
      timeframe={timeframe}
      onTimeframeChange={setTimeframe}
      selectedAnnotationId={selectedAnnotationId}
      onSelectAnnotation={setSelectedAnnotationId}
      activeTab={activeTab}
      onTabChange={setActiveTab}
      currency={getChartCurrency(stock.market)}
      chartLoading={chartBarsQuery.isPending}
      chartError={chartBarsQuery.isError}
      annotationState={getStockPageListState(annotationsQuery)}
      noteState={getStockPageListState(notesQuery)}
      tradeState={getStockPageListState(tradesQuery)}
    />
  )
}
