import { useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'

import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import { type Timeframe } from '#components/timeframe-selector'
import {
  addTrendLinePoint,
  type ChartAnnotation,
  type HorizontalLineAnnotationDraft,
  type HorizontalLinePoint,
  type TrendLineAnnotationDraft,
  type TrendLineDrawingProgress,
  type TrendLinePoint,
} from '#lib/annotation-chart-utils'
import {
  deleteChartAnnotation,
  saveHorizontalLineAnnotation,
  saveTrendLineAnnotation,
} from '#lib/annotation-mutation-utils'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
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
type Bar = components['schemas']['Bar']

interface HorizontalLineDrawingState {
  stockId: string
  enabled: boolean
  pending: HorizontalLinePoint | null
}

interface TrendLineDrawingState extends TrendLineDrawingProgress {
  stockId: string
}

function StockChartPage() {
  const { stockId } = Route.useParams()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')
  const [isMarketDepthOpen, setIsMarketDepthOpen] = useState(false)
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<
    string | null
  >(null)
  const [horizontalLineState, setHorizontalLineState] =
    useState<HorizontalLineDrawingState>(() => ({
      stockId,
      enabled: false,
      pending: null,
    }))
  const [horizontalLineSaveError, setHorizontalLineSaveError] = useState<
    string | null
  >(null)
  const [trendLineState, setTrendLineState] = useState<TrendLineDrawingState>(
    () => ({
      stockId,
      enabled: false,
      start: null,
      pending: null,
    }),
  )
  const [trendLineSaveError, setTrendLineSaveError] = useState<string | null>(
    null,
  )
  const [annotationDeleteError, setAnnotationDeleteError] = useState<
    string | null
  >(null)
  const queryClient = useQueryClient()
  const indicators = useChartIndicators()
  const lookbackBars = getChartIndicatorLookbackBars(indicators)
  const displayRange = useMemo(() => getBarsRange(timeframe), [timeframe])
  const chartRange = useMemo(
    () => extendBarsRange(displayRange, timeframe, lookbackBars),
    [displayRange, lookbackBars, timeframe],
  )
  const chartBarsKey = JSON.stringify([
    stockId,
    timeframe,
    displayRange.from,
    displayRange.to,
  ])
  const [cachedChartBars, setCachedChartBars] = useState<{
    key: string
    bars: Bar[]
  } | null>(null)

  const { data, isLoading, error } = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: stockId,
        timeframe,
        ...chartRange,
      },
    },
  })
  useEffect(() => {
    if (data == null) return
    setCachedChartBars({ key: chartBarsKey, bars: data })
  }, [chartBarsKey, data])
  const chartBars =
    data ??
    (cachedChartBars?.key === chartBarsKey ? cachedChartBars.bars : undefined)
  const isCurrentDrawingStock = horizontalLineState.stockId === stockId
  const horizontalLineDrawingEnabled =
    isCurrentDrawingStock && horizontalLineState.enabled
  const pendingHorizontalLine = isCurrentDrawingStock
    ? horizontalLineState.pending
    : null
  const isCurrentTrendLineStock = trendLineState.stockId === stockId
  const trendLineDrawingEnabled =
    isCurrentTrendLineStock && trendLineState.enabled
  const pendingTrendLineStart = isCurrentTrendLineStock
    ? trendLineState.start
    : null
  const pendingTrendLine = isCurrentTrendLineStock
    ? trendLineState.pending
    : null
  const { data: stock } = $api.useQuery('get', '/api/refs/stocks/{id}', {
    params: { path: { id: stockId } },
  })
  const annotationQueryOptions = $api.queryOptions('get', '/api/annotations', {
    params: { query: { target_symbol: stockId } },
  })
  const { data: annotations, isPending: isAnnotationsPending } = useQuery(
    annotationQueryOptions,
  )
  const annotationQueryKey = annotationQueryOptions.queryKey
  const createAnnotationMutation = $api.useMutation(
    'post',
    '/api/annotations',
    {
      onSuccess: (annotation) => {
        queryClient.setQueryData<components['schemas']['Annotation'][]>(
          annotationQueryKey,
          (current) => (current == null ? current : [annotation, ...current]),
        )
        void queryClient.invalidateQueries({ queryKey: annotationQueryKey })
        setSelectedAnnotationId(annotation.id)
        setHorizontalLineState((current) =>
          current.stockId === stockId
            ? { ...current, enabled: false, pending: null }
            : current,
        )
        setTrendLineState((current) =>
          current.stockId === stockId
            ? { ...current, enabled: false, start: null, pending: null }
            : current,
        )
        setHorizontalLineSaveError(null)
        setTrendLineSaveError(null)
      },
      onError: (_, variables) => {
        if (variables.body.price_start != null) {
          setTrendLineSaveError('トレンドラインの保存に失敗しました')
          return
        }

        setHorizontalLineSaveError('水平線の保存に失敗しました')
      },
    },
  )
  const deleteAnnotationMutation = $api.useMutation(
    'delete',
    '/api/annotations/{id}',
    {
      onSuccess: (_, variables) => {
        const deletedId = variables.params.path.id
        queryClient.setQueryData<components['schemas']['Annotation'][]>(
          annotationQueryKey,
          (current) => current?.filter(({ id }) => id !== deletedId),
        )
        void queryClient.invalidateQueries({ queryKey: annotationQueryKey })
        setSelectedAnnotationId(null)
        setAnnotationDeleteError(null)
      },
      onError: () => {
        setAnnotationDeleteError('アノテーションの削除に失敗しました')
      },
    },
  )

  function toggleHorizontalLineDrawing() {
    setHorizontalLineSaveError(null)
    setTrendLineSaveError(null)
    setTrendLineState({ stockId, enabled: false, start: null, pending: null })
    setHorizontalLineState((current) => ({
      stockId,
      enabled: current.stockId === stockId ? !current.enabled : true,
      pending: current.stockId === stockId ? current.pending : null,
    }))
  }

  function cancelHorizontalLine() {
    setHorizontalLineState({ stockId, enabled: false, pending: null })
    setHorizontalLineSaveError(null)
  }

  function placeHorizontalLine(line: HorizontalLinePoint) {
    setHorizontalLineState({ stockId, enabled: false, pending: line })
    setHorizontalLineSaveError(null)
  }

  function toggleTrendLineDrawing() {
    setHorizontalLineState({ stockId, enabled: false, pending: null })
    setHorizontalLineSaveError(null)
    setTrendLineSaveError(null)
    setTrendLineState((current) => {
      const enabled = current.stockId === stockId ? !current.enabled : true
      return {
        stockId,
        enabled,
        start: enabled && current.stockId === stockId ? current.start : null,
        pending: null,
      }
    })
  }

  function placeTrendLinePoint(point: TrendLinePoint) {
    setTrendLineState((current) =>
      current.stockId === stockId
        ? { ...addTrendLinePoint(current, point), stockId }
        : current,
    )
    setTrendLineSaveError(null)
  }

  function cancelTrendLine() {
    setTrendLineState({ stockId, enabled: false, start: null, pending: null })
    setTrendLineSaveError(null)
  }

  function saveHorizontalLine(draft: HorizontalLineAnnotationDraft) {
    if (createAnnotationMutation.isPending) return

    saveHorizontalLineAnnotation(
      createAnnotationMutation.mutate,
      stockId,
      draft,
    )
  }

  function saveTrendLine(draft: TrendLineAnnotationDraft) {
    if (createAnnotationMutation.isPending) return

    saveTrendLineAnnotation(createAnnotationMutation.mutate, stockId, draft)
  }

  function deleteAnnotation(id: string) {
    if (deleteAnnotationMutation.isPending) return

    setAnnotationDeleteError(null)
    deleteChartAnnotation(deleteAnnotationMutation.mutate, id)
  }

  return (
    <StockChartPageView
      stockId={stockId}
      stockName={stock?.name ?? stockId}
      bars={chartBars ?? []}
      displayRangeStart={displayRange.from}
      annotations={annotations ?? emptyAnnotations}
      timeframe={timeframe}
      onTimeframeChange={setTimeframe}
      isMarketDepthOpen={isMarketDepthOpen}
      onToggleMarketDepth={() => {
        setIsMarketDepthOpen((previous) => !previous)
      }}
      selectedAnnotationId={selectedAnnotationId}
      onSelectAnnotation={setSelectedAnnotationId}
      horizontalLineDrawingEnabled={horizontalLineDrawingEnabled}
      pendingHorizontalLine={pendingHorizontalLine}
      isSavingHorizontalLine={createAnnotationMutation.isPending}
      horizontalLineSaveError={horizontalLineSaveError}
      onToggleHorizontalLineDrawing={toggleHorizontalLineDrawing}
      onPlaceHorizontalLine={placeHorizontalLine}
      onSaveHorizontalLine={saveHorizontalLine}
      onCancelHorizontalLine={cancelHorizontalLine}
      trendLineDrawingEnabled={trendLineDrawingEnabled}
      pendingTrendLineStart={pendingTrendLineStart}
      pendingTrendLine={pendingTrendLine}
      isSavingTrendLine={createAnnotationMutation.isPending}
      trendLineSaveError={trendLineSaveError}
      onToggleTrendLineDrawing={toggleTrendLineDrawing}
      onPlaceTrendLinePoint={placeTrendLinePoint}
      onSaveTrendLine={saveTrendLine}
      onCancelTrendLine={cancelTrendLine}
      isDeletingAnnotation={deleteAnnotationMutation.isPending}
      annotationDeleteError={annotationDeleteError}
      onDeleteAnnotation={deleteAnnotation}
      currency={getChartCurrency(stock?.market)}
      isLoading={isLoading && chartBars == null}
      hasError={error != null && chartBars == null}
      isAnnotationsLoading={isAnnotationsPending}
    />
  )
}
