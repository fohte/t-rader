import { Button } from '@fohte/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@fohte/ui/tooltip'
import { createFileRoute } from '@tanstack/react-router'
import { Columns2Icon } from 'lucide-react'
import { useMemo, useState } from 'react'

import { CandlestickChart } from '#components/candlestick-chart'
import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'
import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import { getBarsRange } from '#lib/chart-range'
import { getChartCurrency } from '#lib/chart-utils'

export const Route = createFileRoute('/charts/$instrumentId')({
  component: ChartPage,
})

function ChartPage() {
  const { instrumentId } = Route.useParams()
  const [timeframe, setTimeframe] = useState<Timeframe>('1d')
  const [isMarketDepthOpen, setIsMarketDepthOpen] = useState(false)
  const { from, to } = useMemo(() => getBarsRange(timeframe), [timeframe])

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
  const { data: instrument } = $api.useQuery('get', '/api/refs/stocks/{id}', {
    params: { path: { id: instrumentId } },
  })
  const currency = getChartCurrency(instrument?.market)

  const toggleMarketDepth = () => {
    setIsMarketDepthOpen((prev) => !prev)
  }

  const toolbar = (
    <div className="flex items-center gap-2">
      <TimeframeSelector value={timeframe} onChange={setTimeframe} />
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                variant={isMarketDepthOpen ? 'default' : 'outline'}
                size="icon-sm"
                onClick={toggleMarketDepth}
                aria-label="板情報・歩み値パネルの表示切替"
                aria-pressed={isMarketDepthOpen}
              >
                <Columns2Icon />
              </Button>
            }
          />
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
          <Skeleton className="h-150 w-full" />
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
        <CandlestickChart
          bars={data ?? []}
          currency={currency}
          intraday={timeframe !== '1d' && timeframe !== '1w'}
          className="h-150 w-full"
        />
        <ChartMarketDepthPanel
          instrumentId={instrumentId}
          isOpen={isMarketDepthOpen}
          onToggle={toggleMarketDepth}
        />
      </div>
    </div>
  )
}
