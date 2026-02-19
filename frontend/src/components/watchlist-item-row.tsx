import { useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Loader2, Trash2 } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type WatchlistItem = components['schemas']['WatchlistItem']

type WatchlistItemRowViewProps = {
  item: WatchlistItem
  name: string | undefined
  onDelete: () => void
  isDeleting: boolean
}

export function WatchlistItemRowView({
  item,
  name,
  onDelete,
  isDeleting,
}: WatchlistItemRowViewProps) {
  return (
    <div className="flex items-center justify-between rounded-md px-3 py-2 hover:bg-accent">
      <Link
        to="/charts/$instrumentId"
        params={{ instrumentId: item.instrument_id }}
        className="flex flex-1 items-center gap-3"
      >
        <span className="font-mono font-medium">{item.instrument_id}</span>
        {name && <span className="text-sm text-muted-foreground">{name}</span>}
      </Link>
      <Button
        variant="ghost"
        size="icon"
        className="size-8"
        disabled={isDeleting}
        onClick={(e) => {
          e.stopPropagation()
          onDelete()
        }}
      >
        {isDeleting ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : (
          <Trash2 className="size-3.5" />
        )}
      </Button>
    </div>
  )
}

type WatchlistItemRowProps = {
  item: WatchlistItem
  name: string | undefined
  watchlistId: string
}

export function WatchlistItemRow({
  item,
  name,
  watchlistId,
}: WatchlistItemRowProps) {
  const queryClient = useQueryClient()

  const deleteMutation = $api.useMutation(
    'delete',
    '/api/watchlists/{id}/items/{instrument_id}',
    {
      onSuccess: () => {
        queryClient.invalidateQueries({
          queryKey: $api.queryOptions('get', '/api/watchlists/{id}/items', {
            params: { path: { id: watchlistId } },
          }).queryKey,
        })
      },
    },
  )

  return (
    <WatchlistItemRowView
      item={item}
      name={name}
      isDeleting={deleteMutation.isPending}
      onDelete={() => {
        deleteMutation.mutate({
          params: {
            path: { id: watchlistId, instrument_id: item.instrument_id },
          },
        })
      }}
    />
  )
}
