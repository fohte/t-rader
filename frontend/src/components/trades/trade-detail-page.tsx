import { useQueries, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { CreateNoteDialog } from '#components/strategy-home/create-note-dialog'
import { createTradeReflectionDraft } from '#components/trades/trade-detail-model'
import { TradeDetailView } from '#components/trades/trade-detail-view'
import { TradeNotesDialog } from '#components/trades/trade-notes-dialog'
import type { TradeRequestedAnalysis } from '#components/trades/trade-requested-analyses'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import { $api } from '#lib/api/client'

export function TradeDetailPage({ tradeId }: { tradeId: string }) {
  const queryClient = useQueryClient()
  const invalidateTrades = useInvalidateTrades()
  const [notesDialogOpen, setNotesDialogOpen] = useState(false)
  const [reflectionDialogOpen, setReflectionDialogOpen] = useState(false)
  const [noteLinkError, setNoteLinkError] = useState<string | null>(null)

  const detailQuery = $api.useQuery('get', '/api/trades/{id}', {
    params: { path: { id: tradeId } },
  })
  const linkedNotesQuery = $api.useQuery(
    'get',
    '/api/trades/{id}/notes',
    { params: { path: { id: tradeId } } },
    { enabled: detailQuery.data != null },
  )
  const strategiesQuery = $api.useQuery('get', '/api/strategies')
  const stocksQuery = $api.useQuery('get', '/api/refs/stocks')
  const linkMutation = $api.useMutation('post', '/api/trades/{id}/notes')

  const strategyTaskQueries = useQueries({
    queries: (strategiesQuery.data ?? []).map((strategy) => ({
      ...$api.queryOptions('get', '/api/strategies/{id}/tasks', {
        params: {
          path: { id: strategy.id },
          query: { context_kind: 'trade', context_id: tradeId },
        },
      }),
      enabled: detailQuery.data != null,
    })),
  })
  const tasks = strategyTaskQueries
    .flatMap((query) => query.data ?? [])
    .sort((a, b) => b.created_at.localeCompare(a.created_at))
  const taskNotesQueries = useQueries({
    queries: tasks.map((task) =>
      $api.queryOptions('get', '/api/strategies/{id}/tasks/{task_id}/notes', {
        params: {
          path: { id: task.strategy_id, task_id: task.task_id },
        },
      }),
    ),
  })
  const requestedAnalyses: TradeRequestedAnalysis[] = tasks.map(
    (task, index) => {
      const notesQuery = taskNotesQueries[index]
      return {
        strategyId: task.strategy_id,
        taskId: task.task_id,
        prompt: task.prompt,
        phase: task.phase,
        createdAt: task.created_at,
        notes: (notesQuery?.data ?? []).map((note) => ({
          id: note.id,
          title: note.title,
          versionId: note.version_id,
        })),
        notesState:
          notesQuery?.isPending === true
            ? 'loading'
            : notesQuery?.isError === true
              ? 'error'
              : 'success',
      }
    },
  )
  const requestedAnalysesState =
    strategiesQuery.isPending ||
    strategyTaskQueries.some((query) => query.isPending)
      ? 'loading'
      : strategiesQuery.isError ||
          strategyTaskQueries.some((query) => query.isError)
        ? 'error'
        : 'success'

  if (detailQuery.isPending) {
    return <TradeDetailView state="loading" />
  }

  const detail = detailQuery.data
  if (detail == null) {
    return <TradeDetailView state="error" />
  }

  const stockName =
    stocksQuery.data?.find((stock) => stock.id === detail.symbol)?.name ?? null
  const strategyName =
    strategiesQuery.data?.find((strategy) => strategy.id === detail.strategy_id)
      ?.name ?? null
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
