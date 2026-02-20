import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'

import { AddInstrumentForm } from '@/components/add-instrument-form'
import { Separator } from '@/components/ui/separator'
import { Skeleton } from '@/components/ui/skeleton'
import { WatchlistItemList } from '@/components/watchlist-item-list'
import { WatchlistSelector } from '@/components/watchlist-selector'
import { useInstrumentNames } from '@/hooks/use-instrument-names'
import { $api } from '@/lib/api/client'

export const Route = createFileRoute('/')({
  component: WatchlistPage,
})

function WatchlistPage() {
  const { data: watchlists, isPending } = $api.useQuery(
    'get',
    '/api/watchlists',
  )
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const { names, registerName } = useInstrumentNames()
  const queryClient = useQueryClient()

  // デフォルトウォッチリスト自動作成の重複防止
  const isAutoCreatingRef = useRef(false)

  const createMutation = $api.useMutation('post', '/api/watchlists')

  // ウォッチリストが 0 件の場合、「デフォルト」を自動作成
  useEffect(() => {
    if (watchlists && watchlists.length === 0 && !isAutoCreatingRef.current) {
      isAutoCreatingRef.current = true
      createMutation.mutate(
        { body: { name: 'デフォルト' } },
        {
          onSuccess: (data) => {
            queryClient.invalidateQueries({
              queryKey: $api.queryOptions('get', '/api/watchlists').queryKey,
            })
            setSelectedId(data.id)
          },
        },
      )
    }
  }, [watchlists, createMutation, queryClient])

  // ウォッチリスト読み込み後、最初のウォッチリストを選択
  useEffect(() => {
    const first = watchlists?.[0]
    if (first && !selectedId) {
      setSelectedId(first.id)
    }
  }, [watchlists, selectedId])

  if (isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-9 w-60" />
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <WatchlistSelector
        watchlists={watchlists ?? []}
        selectedId={selectedId}
        onSelect={setSelectedId}
      />

      {selectedId && (
        <>
          <WatchlistItemList watchlistId={selectedId} instrumentNames={names} />
          <Separator />
          <AddInstrumentForm
            watchlistId={selectedId}
            onNameRegistered={registerName}
          />
        </>
      )}
    </div>
  )
}
