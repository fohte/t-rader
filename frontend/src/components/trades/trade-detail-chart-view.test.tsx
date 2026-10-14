import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TRADE_DETAIL_CHART_BARS } from '#components/trades/trade-detail-chart.fixtures'
import { TradeDetailChartView } from '#components/trades/trade-detail-chart-view'
import type { components } from '#lib/api/schema.gen'
import type { TradeMarker } from '#lib/chart-utils'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Bar = components['schemas']['Bar']

vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: ({
    bars,
    trades,
    selectedTradeId,
    priceScaleTopMargin,
  }: {
    bars: Bar[]
    trades?: TradeMarker[]
    selectedTradeId?: string
    priceScaleTopMargin?: number
  }) => (
    <div role="img" aria-label="日足チャート">
      {JSON.stringify({
        barDates: bars.map((bar) => bar.timestamp),
        tradeIds: trades?.map((item) => item.id),
        selectedTradeId,
        priceScaleTopMargin,
      })}
    </div>
  ),
}))

const trade: TradeDetail = {
  id: 'fake-trade-buy',
  strategy_id: 'fake-strategy',
  symbol: 'FIC-01',
  side: 'buy',
  qty: 10,
  price: 1000,
  fee: 10,
  date: '2099-02-10',
  source: 'manual',
  note: null,
  created_at: '2099-02-10T00:00:00Z',
  updated_at: '2099-02-10T00:00:00Z',
  remaining_qty: 10,
}

const tradeMarker: TradeMarker = {
  id: trade.id,
  side: trade.side,
  date: trade.date,
}

afterEach(cleanup)

describe('TradeDetailChartView', () => {
  it('shows a loading placeholder and the selected trade summary', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'loading' }} />)

    const actual = JSON.stringify([
      screen.getByRole('status').getAttribute('aria-label'),
      screen.getByText('対象取引: 2099-02-10 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日のチャートを読み込み中',
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })

  it('shows an error when the chart data cannot be loaded', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'error' }} />)

    const actual = JSON.stringify([
      screen.getByRole('alert').textContent,
      screen.getByText('対象取引: 2099-02-10 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日のチャートを読み込めませんでした。',
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })

  it('shows an empty state when no daily bars are available', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'empty' }} />)

    const actual = JSON.stringify([
      screen.getByRole('status').textContent,
      screen.getByText('対象取引: 2099-02-10 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日の前後に表示できる日足がありません。',
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })

  it('passes the daily bars, selected trade, and marker spacing to the chart', () => {
    render(
      <TradeDetailChartView
        trade={trade}
        state={{
          status: 'ready',
          bars: TRADE_DETAIL_CHART_BARS,
          trades: [tradeMarker],
        }}
      />,
    )

    const actual = JSON.stringify([
      screen.getByRole('img', { name: '日足チャート' }).textContent,
      screen.getByText('対象取引: 2099-02-10 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        JSON.stringify({
          barDates: [
            '2099-02-09T00:00:00Z',
            '2099-02-10T00:00:00Z',
            '2099-02-11T00:00:00Z',
          ],
          tradeIds: ['fake-trade-buy'],
          selectedTradeId: 'fake-trade-buy',
          priceScaleTopMargin: 0.25,
        }),
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })

  it('shows a legend for buy and sell markers', () => {
    render(
      <TradeDetailChartView
        trade={trade}
        state={{
          status: 'ready',
          bars: TRADE_DETAIL_CHART_BARS,
          trades: [tradeMarker],
        }}
      />,
    )

    expect(
      screen.getByRole('group', { name: '取引マーカーの凡例' }).textContent,
    ).toBe('▲買 ▼売')
  })
})
