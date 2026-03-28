import { createFileRoute } from '@tanstack/react-router'
import { Columns2Icon } from 'lucide-react'
import { useState } from 'react'

import { CandlestickChart } from '@/components/candlestick-chart'
import { ChartMarketDepthPanel } from '@/components/chart-market-depth-panel'
import {
  type Timeframe,
  TimeframeSelector,
} from '@/components/timeframe-selector'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
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
  const [isMarketDepthOpen, setIsMarketDepthOpen] = useState(false)

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

  const toggleMarketDepth = () => {
    setIsMarketDepthOpen((prev) => !prev)
  }

  const toolbar = (
    <div className="flex items-center gap-2">
      <TimeframeSelector value={timeframe} onChange={setTimeframe} />
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={isMarketDepthOpen ? 'default' : 'outline'}
              size="icon-sm"
              onClick={toggleMarketDepth}
              aria-label="板情報・歩み値パネルの表示切替"
              aria-pressed={isMarketDepthOpen}
            >
              <Columns2Icon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>板情報・歩み値</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  )

  if (isLoading) {
    return (
      <div className="flex h-full flex-col gap-4">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
          {toolbar}
        </div>
        <div className="flex min-h-0 flex-1 gap-4">
          <Skeleton className="h-[600px] w-full" />
          <ChartMarketDepthPanel
            instrumentId={instrumentId}
            isOpen={isMarketDepthOpen}
            onToggle={toggleMarketDepth}
          />
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full flex-col gap-4">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
          {toolbar}
        </div>
        <div className="flex min-h-0 flex-1 gap-4">
          <p className="text-destructive">データの取得に失敗しました</p>
          <ChartMarketDepthPanel
            instrumentId={instrumentId}
            isOpen={isMarketDepthOpen}
            onToggle={toggleMarketDepth}
          />
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">チャート: {instrumentId}</h1>
        {toolbar}
      </div>
      <div className="flex min-h-0 flex-1 gap-4">
        <CandlestickChart bars={data ?? []} className="h-[600px] w-full" />
        <ChartMarketDepthPanel
          instrumentId={instrumentId}
          isOpen={isMarketDepthOpen}
          onToggle={toggleMarketDepth}
        />
      </div>
    </div>
  )
}

/** Date を YYYY-MM-DD 形式にフォーマットする */
function formatDate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${String(y)}-${m}-${d}`
}
