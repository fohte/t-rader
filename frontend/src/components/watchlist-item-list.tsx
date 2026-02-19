import { PackageOpen } from 'lucide-react'

import { Skeleton } from '@/components/ui/skeleton'
import { WatchlistItemRow } from '@/components/watchlist-item-row'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type WatchlistItem = components['schemas']['WatchlistItem']

type WatchlistItemListViewProps = {
  items: WatchlistItem[]
  instrumentNames: Map<string, string>
  watchlistId: string
}

export function WatchlistItemListView({
  items,
  instrumentNames,
  watchlistId,
}: WatchlistItemListViewProps) {
  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 py-12 text-muted-foreground">
        <PackageOpen className="size-10" />
        <p>銘柄が登録されていません</p>
        <p className="text-sm">下のフォームから銘柄を追加してください</p>
      </div>
    )
  }

  return (
    <div className="divide-y">
      {items.map((item) => (
        <WatchlistItemRow
          key={item.instrument_id}
          item={item}
          name={instrumentNames.get(item.instrument_id)}
          watchlistId={watchlistId}
        />
      ))}
    </div>
  )
}

function WatchlistItemListSkeleton() {
  return (
    <div className="space-y-2">
      {Array.from({ length: 3 }, (_, i) => (
        <Skeleton key={i} className="h-10 w-full" />
      ))}
    </div>
  )
}

type WatchlistItemListProps = {
  watchlistId: string
  instrumentNames: Map<string, string>
}

export function WatchlistItemList({
  watchlistId,
  instrumentNames,
}: WatchlistItemListProps) {
  const { data, isPending, error } = $api.useQuery(
    'get',
    '/api/watchlists/{id}/items',
    {
      params: { path: { id: watchlistId } },
    },
  )

  if (isPending) {
    return <WatchlistItemListSkeleton />
  }

  if (error) {
    return (
      <div className="py-8 text-center text-destructive">
        銘柄一覧の取得に失敗しました
      </div>
    )
  }

  return (
    <WatchlistItemListView
      items={data}
      instrumentNames={instrumentNames}
      watchlistId={watchlistId}
    />
  )
}
