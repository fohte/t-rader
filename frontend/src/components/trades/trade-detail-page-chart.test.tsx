import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TRADE_DETAIL_CHART_BARS } from '#components/trades/trade-detail-chart.fixtures'
import type { components } from '#lib/api/schema.gen'
import type { TradeMarker } from '#lib/chart-utils'
import { createStoryRouter } from '#storybook/story-router'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Trade = components['schemas']['TradeListItem']

vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: ({
    bars,
    trades,
    selectedTradeId,
    currency,
  }: {
    bars: components['schemas']['Bar'][]
    trades?: TradeMarker[]
    selectedTradeId?: string
    currency?: string
  }) => (
    <div role="img" aria-label="日足チャート">
      {JSON.stringify({
        barDates: bars.map((bar) => bar.timestamp),
        tradeIds: trades?.map((trade) => trade.id),
        selectedTradeId,
        currency,
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

const trades: Trade[] = [
  {
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
  },
  {
    id: 'fake-trade-sell',
    strategy_id: trade.strategy_id,
    symbol: trade.symbol,
    side: 'sell',
    qty: 5,
    price: 1100,
    fee: 5,
    date: '2099-02-11',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2099-02-11T00:00:00Z',
    updated_at: '2099-02-11T00:00:00Z',
  },
]

let getBarsResponse = () => Response.json(TRADE_DETAIL_CHART_BARS)

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  getBarsResponse = () => Response.json(TRADE_DETAIL_CHART_BARS)
})

async function renderTradeDetailPageChart(barsResponse?: () => Response) {
  if (barsResponse != null) getBarsResponse = barsResponse

  const requests: Array<{ path: string; query: Record<string, string> }> = []

  vi.stubGlobal('fetch', (input: RequestInfo | URL, init?: RequestInit) => {
    const request = input instanceof Request ? input : new Request(input, init)
    const url = new URL(request.url)
    const path = url.pathname
    const query = Object.fromEntries(url.searchParams.entries())
    requests.push({ path, query })

    if (path === `/api/trades/${trade.id}`) return Response.json(trade)
    if (path === `/api/trades/${trade.id}/notes`) return Response.json([])
    if (path === '/api/trades') return Response.json(trades)
    if (path === '/api/bars') return getBarsResponse()
    if (path === '/api/strategies') return Response.json([])
    if (path === '/api/refs/stocks') {
      return Response.json([
        {
          id: trade.symbol,
          name: '架空銘柄',
          market: '架空市場',
          sector_id: null,
          created_at: '2099-01-01T00:00:00Z',
          updated_at: '2099-01-01T00:00:00Z',
        },
      ])
    }

    return Response.json({})
  })

  const { TradeDetailPage } =
    await import('#components/trades/trade-detail-page')
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createStoryRouter(
    () => <TradeDetailPage tradeId={trade.id} />,
    {
      paths: ['/trades', '/trades/$tradeId', '/stocks/$stockId'],
    },
  )
  render(<RouterProvider router={router} />, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  })

  return requests
}

describe('TradeDetailPage chart', () => {
  it('loads the selected trade date range and marks the current trade', async () => {
    const requests = await renderTradeDetailPageChart()
    const chart = await screen.findByRole('img', { name: '日足チャート' })
    const chartRequests = requests
      .filter(({ path }) => path === '/api/bars' || path === '/api/trades')
      .sort((left, right) => left.path.localeCompare(right.path))

    const actual = JSON.stringify([
      chart.textContent,
      chartRequests.map(({ path, query }) => [
        path,
        Object.entries(query).sort(([left], [right]) =>
          left.localeCompare(right),
        ),
      ]),
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
          tradeIds: ['fake-trade-buy', 'fake-trade-sell'],
          selectedTradeId: 'fake-trade-buy',
          currency: 'JPY',
        }),
        [
          [
            '/api/bars',
            [
              ['from', '2099-01-27'],
              ['instrument_id', 'FIC-01'],
              ['timeframe', '1d'],
              ['to', '2099-02-24'],
            ],
          ],
          ['/api/trades', [['symbol', 'FIC-01']]],
        ],
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })

  it('shows the chart error when the daily bars request fails', async () => {
    await renderTradeDetailPageChart(() => Response.json({}, { status: 500 }))

    const alert = await screen.findByRole('alert')

    expect(
      JSON.stringify([
        alert.textContent,
        screen.getByText('対象取引: 2099-02-10 買い 10 株').textContent,
      ]),
    ).toBe(
      JSON.stringify([
        '取引日のチャートを読み込めませんでした。',
        '対象取引: 2099-02-10 買い 10 株',
      ]),
    )
  })
})
