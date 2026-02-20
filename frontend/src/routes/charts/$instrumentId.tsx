import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

import { CandlestickChart } from '@/components/candlestick-chart'
import {
  TimeframeSelector,
  type Timeframe,
} from '@/components/timeframe-selector'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'

export const Route = createFileRoute('/charts/$instrumentId')({
  component: ChartPage,
})

/** タイムフレームに応じた取得期間 (日数) を返す */
function getLookbackDays(timeframe: Timeframe): number {
  switch (timeframe) {
    case '5m':
    case '15m':
      return 7
    case '1h':
      return 30
    case '4h':
      return 90
    case '1d':
      return 365
    case '1w':
      return 365 * 3
  }
}

function ChartPage() {
  const { instrumentId } = Route.useParams()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')

  const today = new Date()
  const fromDate = new Date(today)
  fromDate.setDate(today.getDate() - getLookbackDays(timeframe))

  const from = formatDate(fromDate)
  const to = formatDate(today)

  const { data, isLoading, error } = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: instrumentId,
        timeframe,
        from,
        to,
      },
    },
  })

  if (isLoading) {
    return (
      <div className="flex h-full flex-col gap-4">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
          <TimeframeSelector value={timeframe} onChange={setTimeframe} />
        </div>
        <Skeleton className="h-[600px] w-full" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full flex-col gap-4">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
          <TimeframeSelector value={timeframe} onChange={setTimeframe} />
        </div>
        <p className="text-destructive">データの取得に失敗しました</p>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
        <TimeframeSelector value={timeframe} onChange={setTimeframe} />
      </div>
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
