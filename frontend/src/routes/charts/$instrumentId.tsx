import { createFileRoute } from '@tanstack/react-router'

import { CandlestickChart } from '@/components/candlestick-chart'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'

export const Route = createFileRoute('/charts/$instrumentId')({
  component: ChartPage,
})

function ChartPage() {
  const { instrumentId } = Route.useParams()

  // 過去 1 年分の日足データを取得
  const today = new Date()
  const oneYearAgo = new Date(today)
  oneYearAgo.setFullYear(today.getFullYear() - 1)

  const from = formatDate(oneYearAgo)
  const to = formatDate(today)

  const { data, isLoading, error } = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: instrumentId,
        timeframe: '1d',
        from,
        to,
      },
    },
  })

  if (isLoading) {
    return (
      <div className="flex h-full flex-col gap-4">
        <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
        <Skeleton className="h-[600px] w-full" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full flex-col gap-4">
        <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
        <p className="text-destructive">データの取得に失敗しました</p>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-4">
      <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
      <CandlestickChart bars={data ?? []} className="h-[600px] w-full" />
    </div>
  )
}

/** Date を YYYY-MM-DD 形式にフォーマットする */
function formatDate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}
