import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'

import { StockPageState } from '#components/stock-page/stock-page-state'
import { StockPageView } from '#components/stock-page/stock-page-view'
import { useChatContext } from '#components/strategy-shell/floating-chat-store'
import { type Timeframe } from '#components/timeframe-selector'
import { $api, fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import {
  getChartIndicatorLookbackBars,
  useChartIndicators,
} from '#lib/chart-indicator-settings'
import { extendBarsRange, getBarsRange } from '#lib/chart-range'
import { getChartCurrency } from '#lib/chart-utils'
import {
  findDailyBarForDate,
  getDailyBarsRangeAroundDate,
  validateStockPageSearch,
} from '#lib/stock-page-date'
import {
  buildStockQuote,
  getStockPageListState,
  resolveStockLookup,
} from '#lib/stock-page-model'

export const Route = createFileRoute('/stocks/$stockId')({
  validateSearch: validateStockPageSearch,
  component: StockPage,
})

type Bar = components['schemas']['Bar']

function StockPage() {
  const { stockId } = Route.useParams()
  const search = Route.useSearch()
  const requestedDate = search.date
  const selectedPeriod =
    search.periodEnd != null && search.periodType != null
      ? { end: search.periodEnd, type: search.periodType }
      : undefined
  const activeTab =
    search.stockTab ?? (selectedPeriod == null ? 'annotations' : 'financials')
  const navigate = Route.useNavigate()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<
    string | null
  >(null)
  const chartIndicators = useChartIndicators()
  const lookbackBars = getChartIndicatorLookbackBars(chartIndicators)
  const displayChartRange = useMemo(() => {
    if (timeframe === '1d' && requestedDate != null) {
      return (
        getDailyBarsRangeAroundDate(requestedDate) ?? getBarsRange(timeframe)
      )
    }
    return getBarsRange(timeframe)
  }, [requestedDate, timeframe])
  const chartRange = useMemo(
    () => extendBarsRange(displayChartRange, timeframe, lookbackBars),
    [displayChartRange, lookbackBars, timeframe],
  )
  const chartBarsKey = JSON.stringify([
    stockId,
    timeframe,
    displayChartRange.from,
    displayChartRange.to,
  ])
  const [cachedChartBars, setCachedChartBars] = useState<{
    key: string
    bars: Bar[]
  } | null>(null)
  const dailyRange = useMemo(() => getBarsRange('1d'), [])
  const selectedDateRange = useMemo(
    () =>
      requestedDate == null ? null : getDailyBarsRangeAroundDate(requestedDate),
    [requestedDate],
  )

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
  useChatContext({
    kind: 'stock',
    id: stockId,
    label:
      stockQuery.data?.kind === 'found'
        ? stockQuery.data.stock.name
        : undefined,
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
  useEffect(() => {
    if (chartBarsQuery.data == null) return
    setCachedChartBars({ key: chartBarsKey, bars: chartBarsQuery.data })
  }, [chartBarsKey, chartBarsQuery.data])
  const chartBars =
    chartBarsQuery.data ??
    (cachedChartBars?.key === chartBarsKey ? cachedChartBars.bars : undefined)
  const dailyBarsQuery = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: stockId,
        timeframe: '1d',
        ...dailyRange,
      },
    },
  })
  const selectedDateBarsQuery = $api.useQuery(
    'get',
    '/api/bars',
    {
      params: {
        query: {
          instrument_id: stockId,
          timeframe: '1d',
          ...(selectedDateRange ?? dailyRange),
        },
      },
    },
    { enabled: requestedDate != null && timeframe !== '1d' },
  )
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
  const groupsQuery = $api.useQuery('get', '/api/stocks/{stock_id}/groups', {
    params: { path: { stock_id: stockId } },
  })
  const nextEarningsQuery = $api.useQuery(
    'get',
    '/api/stocks/{stock_id}/next-earnings',
    { params: { path: { stock_id: stockId } } },
  )
  const valuationQuery = $api.useQuery(
    'get',
    '/api/stocks/{stock_id}/valuation',
    { params: { path: { stock_id: stockId } } },
  )
  const financialSummariesQuery = $api.useQuery(
    'get',
    '/api/stocks/{stock_id}/financial-summaries',
    {
      params: {
        path: { stock_id: stockId },
        query:
          selectedPeriod == null
            ? {}
            : {
                current_period_end: selectedPeriod.end,
                current_period_type: selectedPeriod.type,
              },
      },
    },
  )
  const selectedDateBar = useMemo(
    () =>
      requestedDate == null
        ? null
        : findDailyBarForDate(
            (timeframe === '1d' ? chartBars : selectedDateBarsQuery.data) ?? [],
            requestedDate,
          ),
    [chartBars, requestedDate, selectedDateBarsQuery.data, timeframe],
  )
  const selectedDateLoading =
    requestedDate != null &&
    (timeframe === '1d'
      ? chartBarsQuery.isPending
      : selectedDateBarsQuery.isPending)
  const selectedDateError =
    requestedDate != null &&
    (timeframe === '1d'
      ? chartBarsQuery.isError
      : selectedDateBarsQuery.isError)

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
      bars={chartBars ?? []}
      displayRangeStart={displayChartRange.from}
      annotations={annotationsQuery.data ?? []}
      notes={notesQuery.data ?? []}
      trades={tradesQuery.data ?? []}
      groups={groupsQuery.data ?? []}
      groupsState={getStockPageListState(groupsQuery)}
      nextEarnings={nextEarningsQuery.data ?? null}
      nextEarningsState={getStockPageListState(nextEarningsQuery)}
      valuation={valuationQuery.data ?? null}
      valuationState={getStockPageListState(valuationQuery)}
      financialSummaries={financialSummariesQuery.data ?? []}
      financialSummariesState={getStockPageListState(financialSummariesQuery)}
      selectedPeriod={selectedPeriod}
      position={position}
      holdingsState={getStockPageListState(summaryQuery)}
      timeframe={timeframe}
      onTimeframeChange={setTimeframe}
      selectedAnnotationId={selectedAnnotationId}
      onSelectAnnotation={setSelectedAnnotationId}
      activeTab={activeTab}
      onTabChange={(tab) => {
        void navigate({
          search: (previous) => ({ ...previous, stockTab: tab }),
        })
      }}
      currency={getChartCurrency(stock.market)}
      chartLoading={chartBarsQuery.isPending && chartBars == null}
      chartError={chartBarsQuery.isError && chartBars == null}
      selectedDate={requestedDate}
      selectedDateLoading={selectedDateLoading}
      selectedDateError={selectedDateError}
      selectedDateBar={selectedDateBar}
      annotationState={getStockPageListState(annotationsQuery)}
      noteState={getStockPageListState(notesQuery)}
      tradeState={getStockPageListState(tradesQuery)}
    />
  )
}
