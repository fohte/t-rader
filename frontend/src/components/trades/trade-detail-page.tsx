import { useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import {
  openFloatingChat,
  useChatContext,
} from '#components/strategy-shell/floating-chat-store'
import {
  createTradeDetailChartState,
  createTradeReflectionDraft,
  formatTradeContextLabel,
  getTradeDetailChartRange,
} from '#components/trades/trade-detail-model'
import { TradeDetailView } from '#components/trades/trade-detail-view'
import { TradeNotesDialog } from '#components/trades/trade-notes-dialog'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import { useTradeRequestedAnalyses } from '#components/trades/use-trade-requested-analyses'
import { $api } from '#lib/api/client'
import { getChartCurrency } from '#lib/chart-utils'

export function TradeDetailPage({ tradeId }: { tradeId: string }) {
  const queryClient = useQueryClient()
  const invalidateTrades = useInvalidateTrades()
  const [notesDialogOpen, setNotesDialogOpen] = useState(false)
  const [reflectionDialogOpen, setReflectionDialogOpen] = useState(false)
  const [noteLinkError, setNoteLinkError] = useState<string | null>(null)

  const detailQuery = $api.useQuery('get', '/api/trades/{id}', {
    params: { path: { id: tradeId } },
  })
  const chartRange = getTradeDetailChartRange(detailQuery.data?.date)
  const chartBarsQuery = $api.useQuery(
    'get',
    '/api/bars',
    {
      params: {
        query: {
          instrument_id: detailQuery.data?.symbol ?? '',
          timeframe: '1d',
          from: chartRange?.from ?? '',
          to: chartRange?.to ?? '',
        },
      },
    },
    { enabled: chartRange != null },
  )
  const chartTradesQuery = $api.useQuery(
    'get',
    '/api/trades',
    { params: { query: { symbol: detailQuery.data?.symbol ?? '' } } },
    { enabled: detailQuery.data != null },
  )
  const linkedNotesQuery = $api.useQuery(
    'get',
    '/api/trades/{id}/notes',
    { params: { path: { id: tradeId } } },
    { enabled: detailQuery.data != null },
  )
  const strategiesQuery = $api.useQuery('get', '/api/strategies')
  const stocksQuery = $api.useQuery('get', '/api/refs/stocks')
  const linkMutation = $api.useMutation('post', '/api/trades/{id}/notes')
  const detail = detailQuery.data
  const stock = stocksQuery.data?.find((item) => item.id === detail?.symbol)
  const stockName = stock?.name ?? null
  useChatContext(
    detail == null
      ? null
      : {
          kind: 'trade',
          id: detail.id,
          label: formatTradeContextLabel(detail, stockName),
          strategyId: detail.strategy_id,
        },
  )

  const { analyses: requestedAnalyses, state: requestedAnalysesState } =
    useTradeRequestedAnalyses({
      tradeId,
      strategies: strategiesQuery.data,
      strategyState: strategiesQuery.isPending
        ? 'loading'
        : strategiesQuery.isError
          ? 'error'
          : 'success',
      enabled: detailQuery.data != null,
    })

  if (detailQuery.isPending) {
    return <TradeDetailView state="loading" />
  }

  if (detail == null) {
    return <TradeDetailView state="error" />
  }

  const strategyName =
    strategiesQuery.data?.find((strategy) => strategy.id === detail.strategy_id)
      ?.name ?? null
  const chartState = createTradeDetailChartState({
    bars: chartBarsQuery.data,
    barsPending: chartBarsQuery.isPending,
    barsError: chartBarsQuery.isError,
    trades: chartTradesQuery.data,
    selectedTrade: detail,
  })
  const reflectionDraft = createTradeReflectionDraft(detail, stockName)

  function handleCreated(noteId: string) {
    setNoteLinkError(null)
    linkMutation.mutate(
      { params: { path: { id: tradeId } }, body: { note_id: noteId } },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/trades/{id}/notes', {
              params: { path: { id: tradeId } },
            }).queryKey,
          })
          invalidateTrades()
        },
        onError: () => {
          setNoteLinkError(
            'ノートを作成しましたが、この取引への紐付けに失敗しました。既存のノートを紐付ける操作から追加してください。',
          )
        },
      },
    )
  }

  return (
    <>
      <TradeDetailView
        state="ready"
        trade={detail}
        chartCurrency={getChartCurrency(stock?.market)}
        chartState={chartState}
        stockName={stockName}
        strategyName={strategyName}
        linkedNotes={linkedNotesQuery.data ?? []}
        notesPending={linkedNotesQuery.isPending}
        notesError={linkedNotesQuery.isError}
        requestedAnalyses={requestedAnalyses}
        requestedAnalysesState={requestedAnalysesState}
        noteLinkError={noteLinkError}
        onManageNotes={() => {
          setNotesDialogOpen(true)
        }}
        onWriteReflection={() => {
          setNoteLinkError(null)
          setReflectionDialogOpen(true)
        }}
        onAskAgent={() => {
          openFloatingChat(
            'この取引について、売買時の判断とその後の結果を振り返ってください。',
          )
        }}
      />
      <TradeNotesDialog
        open={notesDialogOpen}
        onOpenChange={setNotesDialogOpen}
        trade={detail}
      />
      <CreateNoteDialog
        open={reflectionDialogOpen}
        onOpenChange={setReflectionDialogOpen}
        initialTitle={reflectionDraft.title}
        initialBody={reflectionDraft.body}
        showTrigger={false}
        onCreated={handleCreated}
      />
    </>
  )
}
