import type { CSSProperties } from 'react'

import { CandlestickChart } from '#components/candlestick-chart'
import type { TradeDetailChartState } from '#components/trades/trade-detail-model'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { type ChartCurrency, TRADE_MARKER_COLORS } from '#lib/chart-utils'

type TradeDetail = components['schemas']['TradeDetailResponse']
type TradeMarkerColorStyle = CSSProperties & {
  '--trade-marker-color': string
}
const buyMarkerStyle: TradeMarkerColorStyle = {
  '--trade-marker-color': TRADE_MARKER_COLORS.buy,
}
const sellMarkerStyle: TradeMarkerColorStyle = {
  '--trade-marker-color': TRADE_MARKER_COLORS.sell,
}

export function TradeDetailChartView({
  trade,
  currency,
  state,
}: {
  trade: TradeDetail
  currency?: ChartCurrency
  state: TradeDetailChartState
}) {
  return (
    <section
      aria-labelledby="trade-detail-chart-title"
      className="min-w-0 space-y-2"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2
          id="trade-detail-chart-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          取引日のチャート
        </h2>
        <p className="font-mono text-2xs text-muted-foreground-strong">
          対象取引: {trade.date} {trade.side === 'buy' ? '買い' : '売り'}{' '}
          {trade.qty.toLocaleString()} 株
        </p>
      </div>
      {state.status === 'ready' ? (
        <>
          <CandlestickChart
            bars={state.bars}
            trades={state.trades}
            selectedTradeId={trade.id}
            currency={currency}
            priceScaleTopMargin={0.25}
            className="h-60 w-full min-w-0 border border-border bg-card"
          />
          <p
            aria-label="取引マーカーの凡例"
            className="flex gap-3 font-mono text-2xs"
            role="group"
          >
            <span
              className="text-(--trade-marker-color)"
              style={buyMarkerStyle}
            >
              ▲買
            </span>{' '}
            <span
              className="text-(--trade-marker-color)"
              style={sellMarkerStyle}
            >
              ▼売
            </span>
          </p>
        </>
      ) : state.status === 'loading' ? (
        <div
          aria-label="取引日のチャートを読み込み中"
          role="status"
          className="h-60 w-full animate-pulse border border-border bg-surface-strong"
        >
          <Skeleton className="h-full w-full" />
        </div>
      ) : (
        <p
          role={state.status === 'error' ? 'alert' : 'status'}
          className="flex h-60 items-center justify-center border border-border bg-card px-4 text-sm text-muted-foreground"
        >
          {state.status === 'empty'
            ? '取引日の前後に表示できる日足がありません。'
            : '取引日のチャートを読み込めませんでした。'}
        </p>
      )}
    </section>
  )
}
