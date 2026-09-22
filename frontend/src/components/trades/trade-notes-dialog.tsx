import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'

import { TradeNotesDialogView } from '#components/trades/trade-notes-dialog-view'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Trade = components['schemas']['Trade']

export function TradeNotesDialog({
  open,
  onOpenChange,
  trade,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  trade: Trade | null
}) {
  const queryClient = useQueryClient()
  const invalidateTrades = useInvalidateTrades()
  const [search, setSearch] = useState('')
  const [operationError, setOperationError] = useState<string | null>(null)
  const tradeId = trade?.id ?? ''
  const strategyId = trade?.strategy_id ?? ''

  useEffect(() => {
    if (open) {
      setSearch('')
      setOperationError(null)
    }
  }, [open, tradeId])

  const linkedNotesQuery = $api.useQuery(
    'get',
    '/api/trades/{id}/notes',
    { params: { path: { id: tradeId } } },
    { enabled: open && trade != null },
  )
  const candidateNotesQuery = $api.useQuery(
    'get',
    '/api/notes',
    { params: { query: { strategy_id: strategyId } } },
    { enabled: open && trade != null },
  )

  const linkMutation = $api.useMutation('post', '/api/trades/{id}/notes')
  const unlinkMutation = $api.useMutation(
    'delete',
    '/api/trades/{id}/notes/{note_id}',
  )
  const linkedNotes = linkedNotesQuery.data ?? []
  const linkedNoteIds = useMemo(
    () => new Set(linkedNotes.map((note) => note.id)),
    [linkedNotes],
  )
  const normalizedSearch = search.trim().toLocaleLowerCase()
  const candidateNotes = useMemo(
    () =>
      (candidateNotesQuery.data ?? []).filter(
        (note) =>
          !linkedNoteIds.has(note.id) &&
          note.title.toLocaleLowerCase().includes(normalizedSearch),
      ),
    [candidateNotesQuery.data, linkedNoteIds, normalizedSearch],
  )

  function invalidateLinkedNotes(id: string) {
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/trades/{id}/notes', {
        params: { path: { id } },
      }).queryKey,
    })
  }

  function handleLink(noteId: string) {
    if (trade == null) return
    setOperationError(null)
    linkMutation.mutate(
      { params: { path: { id: trade.id } }, body: { note_id: noteId } },
      {
        onSuccess: () => {
          invalidateLinkedNotes(trade.id)
          invalidateTrades()
        },
        onError: () => {
          setOperationError('ノートの紐付けに失敗しました。')
        },
      },
    )
  }

  function handleUnlink(noteId: string) {
    if (trade == null) return
    setOperationError(null)
    unlinkMutation.mutate(
      { params: { path: { id: trade.id, note_id: noteId } } },
      {
        onSuccess: () => {
          invalidateLinkedNotes(trade.id)
          invalidateTrades()
        },
        onError: () => {
          setOperationError('ノートの紐付け解除に失敗しました。')
        },
      },
    )
  }

  return (
    <TradeNotesDialogView
      open={open}
      onOpenChange={onOpenChange}
      trade={trade}
      linkedNotes={linkedNotes}
      candidateNotes={candidateNotes}
      search={search}
      onSearchChange={setSearch}
      linkedNotesPending={linkedNotesQuery.isPending}
      candidateNotesPending={candidateNotesQuery.isPending}
      loadingError={linkedNotesQuery.isError || candidateNotesQuery.isError}
      operationError={operationError}
      operationPending={linkMutation.isPending || unlinkMutation.isPending}
      onLink={handleLink}
      onUnlink={handleUnlink}
    />
  )
}
