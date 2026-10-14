import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TradeDetailChartView } from '#components/trades/trade-detail-chart-view'
import type { components } from '#lib/api/schema.gen'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Trade = components['schemas']['TradeListItem']
type Bar = components['schemas']['Bar']

vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: ({
    bars,
    trades,
    selectedTradeId,
  }: {
    bars: Bar[]
    trades?: Trade[]
    selectedTradeId?: string
  }) => (
    <div role="img" aria-label="日足チャート">
      {JSON.stringify({
        barDates: bars.map((bar) => bar.timestamp),
        tradeIds: trades?.map((item) => item.id),
        selectedTradeId,
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
  date: '2030-01-02',
  source: 'manual',
  note: null,
  created_at: '2030-01-02T00:00:00Z',
  updated_at: '2030-01-02T00:00:00Z',
  remaining_qty: 10,
}

const tradeMarker: Trade = {
  id: trade.id,
  strategy_id: trade.strategy_id,
  symbol: trade.symbol,
  side: trade.side,
  qty: trade.qty,
  price: trade.price,
  fee: trade.fee,
  date: trade.date,
  source: trade.source,
  note: trade.note,
  note_count: 0,
  created_at: trade.created_at,
  updated_at: trade.updated_at,
}

const bars: Bar[] = [
  {
    instrument_id: trade.symbol,
    timeframe: '1d',
    timestamp: '2030-01-01T00:00:00Z',
    open: 980,
    high: 1020,
    low: 970,
    close: 1000,
    volume: 1200,
  },
  {
    instrument_id: trade.symbol,
    timeframe: '1d',
    timestamp: '2030-01-02T00:00:00Z',
    open: 1000,
    high: 1030,
    low: 990,
    close: 1010,
    volume: 1400,
  },
]

afterEach(cleanup)

describe('TradeDetailChartView', () => {
  it('shows a loading placeholder and the selected trade summary', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'loading' }} />)

    const actual = JSON.stringify([
      screen.getByRole('status').getAttribute('aria-label'),
      screen.getByText('対象取引: 2030-01-02 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日のチャートを読み込み中',
        '対象取引: 2030-01-02 買い 10 株',
      ]),
    )
  })

  it('shows an error when the chart data cannot be loaded', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'error' }} />)

    const actual = JSON.stringify([
      screen.getByRole('alert').textContent,
      screen.getByText('対象取引: 2030-01-02 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日のチャートを読み込めませんでした。',
        '対象取引: 2030-01-02 買い 10 株',
      ]),
    )
  })

  it('shows an empty state when no daily bars are available', () => {
    render(<TradeDetailChartView trade={trade} state={{ status: 'empty' }} />)

    const actual = JSON.stringify([
      screen.getByRole('status').textContent,
      screen.getByText('対象取引: 2030-01-02 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        '取引日の前後に表示できる日足がありません。',
        '対象取引: 2030-01-02 買い 10 株',
      ]),
    )
  })

  it('passes the daily bars and selected trade to the chart', () => {
    render(
      <TradeDetailChartView
        trade={trade}
        state={{ status: 'ready', bars, trades: [tradeMarker] }}
      />,
    )

    const actual = JSON.stringify([
      screen.getByRole('img', { name: '日足チャート' }).textContent,
      screen.getByText('対象取引: 2030-01-02 買い 10 株').textContent,
    ])

    expect(actual).toBe(
      JSON.stringify([
        JSON.stringify({
          barDates: ['2030-01-01T00:00:00Z', '2030-01-02T00:00:00Z'],
          tradeIds: ['fake-trade-buy'],
          selectedTradeId: 'fake-trade-buy',
        }),
        '対象取引: 2030-01-02 買い 10 株',
      ]),
    )
  })
})
