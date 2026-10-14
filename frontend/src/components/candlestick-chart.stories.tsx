import type { Meta, StoryObj } from '@storybook/react-vite'

import { CandlestickChart } from '#components/candlestick-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Trade = components['schemas']['TradeListItem']

/** [0, 1) の疑似乱数を返す。スクリーンショットの決定性のため Math.random() の代わりに使う */
function createRandom(seed: number): () => number {
  let state = seed
  return () => {
    state = (state * 1103515245 + 12345) & 0x7fffffff
    return state / 0x7fffffff
  }
}

function getSampleBarDate(index: number): Date {
  const date = new Date(2025, 0, 1)
  date.setDate(date.getDate() + index)
  return date
}

/** サンプルデータを生成する */
function generateSampleBars(count: number): Bar[] {
  const random = createRandom(1)
  const bars: Bar[] = []
  let price = 1500

  for (let i = 0; i < count; i++) {
    const date = getSampleBarDate(i)

    const open = price + (random() - 0.5) * 50
    const close = open + (random() - 0.5) * 60
    const high = Math.max(open, close) + random() * 30
    const low = Math.min(open, close) - random() * 30
    const volume = Math.floor(100000 + random() * 500000)

    bars.push({
      instrument_id: 'demo-code',
      timeframe: '1d',
      timestamp: date.toISOString(),
      open: Number(open.toFixed(1)),
      high: Number(high.toFixed(1)),
      low: Number(low.toFixed(1)),
      close: Number(close.toFixed(1)),
      volume,
    })

    price = close
  }

  return bars
}

function generateSampleIntradayBars(count: number): Bar[] {
  const random = createRandom(2)
  const bars: Bar[] = []
  let price = 100

  for (let i = 0; i < count; i++) {
    const open = price + (random() - 0.5) * 2
    const close = open + (random() - 0.5) * 3
    const high = Math.max(open, close) + random()
    const low = Math.min(open, close) - random()

    bars.push({
      instrument_id: 'US:DEMO-A',
      timeframe: '5m',
      timestamp: new Date(Date.UTC(2025, 0, 2, 14, 30 + i * 5)).toISOString(),
      open: Number(open.toFixed(2)),
      high: Number(high.toFixed(2)),
      low: Number(low.toFixed(2)),
      close: Number(close.toFixed(2)),
      volume: Math.floor(100 + random() * 500),
    })

    price = close
  }

  return bars
}

function createTrade({
  id,
  date,
  side,
  price,
  symbol = 'demo-code',
}: {
  id: string
  date: string
  side: 'buy' | 'sell'
  price: number
  symbol?: string
}): Trade {
  const timestamp = `${date}T00:00:00.000Z`
  return {
    id,
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol,
    side,
    qty: 10,
    price,
    fee: 0,
    date,
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: timestamp,
    updated_at: timestamp,
  }
}

const meta = {
  title: 'Components/CandlestickChart',
  component: CandlestickChart,
  args: {
    currency: 'JPY',
  },
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '600px' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CandlestickChart>

const sampleAnnotations: ChartAnnotation[] = [
  {
    id: 'annotation-a',
    target_kind: 'sample-kind',
    status: 'unread',
    timestamp: '2025-01-03T00:00:00.000Z',
    price: 1512,
    text: '価格線を表示するサンプル注釈です。',
  },
  {
    id: 'annotation-b',
    target_kind: 'sample-kind',
    status: 'unread',
    timestamp: '2025-01-03T09:00:00.000Z',
    price: null,
    text: '同じバーに置くサンプル注釈です。',
  },
  {
    id: 'annotation-c',
    target_kind: 'sample-kind',
    status: 'approved',
    timestamp: '2025-01-10T00:00:00.000Z',
    price: null,
    text: '別の日に置くサンプル注釈です。',
  },
]

const periodAnnotation: ChartAnnotation = {
  id: 'annotation-period',
  target_kind: 'sample-kind',
  status: 'unread',
  timestamp_start: '2025-01-04T00:00:00.000Z',
  timestamp: '2025-01-10T00:00:00.000Z',
  price: 1512,
  text: '期間を持つサンプル注釈です。',
}

const periodAnnotationWithoutPrice: ChartAnnotation = {
  ...periodAnnotation,
  id: 'annotation-period-without-price',
  price: null,
  text: '価格を持たない期間注釈です。',
}

const tradeMarkerBars = generateSampleBars(30)
const tradeMarkers: Trade[] = [
  createTrade({
    id: '00000000-0000-0000-0000-000000000201',
    date: getSampleBarDate(2).toISOString().slice(0, 10),
    side: 'buy',
    price: 1512,
  }),
  createTrade({
    id: '00000000-0000-0000-0000-000000000202',
    date: getSampleBarDate(9).toISOString().slice(0, 10),
    side: 'sell',
    price: 1498,
  }),
]
const intradayTradeMarkers: Trade[] = [
  createTrade({
    id: '00000000-0000-0000-0000-000000000203',
    date: '2025-01-02',
    side: 'buy',
    price: 100,
    symbol: 'US:DEMO-A',
  }),
]

export default meta
type Story = StoryObj<typeof meta>

const dateHighlightedBars = generateSampleBars(30)
const dateHighlightedTimestamp = dateHighlightedBars[14]?.timestamp ?? ''

export const Default: Story = {
  name: 'shows a candlestick chart with a full price history.',
  args: {
    bars: generateSampleBars(120),
    className: 'h-full w-full',
  },
}

export const FewBars: Story = {
  name: 'shows a candlestick chart with a short price history.',
  args: {
    bars: generateSampleBars(10),
    className: 'h-full w-full',
  },
}

export const DateHighlighted: Story = {
  name: 'highlights a daily bar with a vertical date marker.',
  args: {
    bars: dateHighlightedBars,
    highlightedTimestamp: dateHighlightedTimestamp,
    className: 'h-full w-full',
  },
}

export const Empty: Story = {
  name: 'shows the empty chart when there are no price bars.',
  args: {
    bars: [],
    className: 'h-full w-full',
  },
}

export const USCurrency: Story = {
  name: 'shows prices in US dollars for a US instrument.',
  args: {
    bars: generateSampleBars(120).map((bar) => ({
      ...bar,
      instrument_id: 'US:DEMO-A',
    })),
    currency: 'USD',
    className: 'h-full w-full',
  },
}

export const Intraday: Story = {
  name: 'shows intraday bars with time labels.',
  args: {
    bars: generateSampleIntradayBars(30),
    currency: 'USD',
    intraday: true,
    className: 'h-full w-full',
  },
}

export const WithTradeMarkers: Story = {
  name: 'shows buy and sell markers on their trade dates.',
  args: {
    bars: tradeMarkerBars,
    trades: tradeMarkers,
    className: 'h-full w-full',
  },
}

export const IntradayWithTradeMarkers: Story = {
  name: 'shows a trade marker on the first bar of its date.',
  args: {
    bars: generateSampleIntradayBars(30),
    trades: intradayTradeMarkers,
    currency: 'USD',
    intraday: true,
    className: 'h-full w-full',
  },
}

export const WithSelectedAnnotation: Story = {
  name: 'shows a selected annotation price line and grouped band marker.',
  args: {
    bars: generateSampleBars(30),
    annotations: sampleAnnotations,
    selectedAnnotationId: 'annotation-a',
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}

export const SelectedAnnotationWithoutPrice: Story = {
  name: 'highlights a selected annotation without a price line.',
  args: {
    bars: generateSampleBars(30),
    annotations: sampleAnnotations,
    selectedAnnotationId: 'annotation-b',
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}

export const PeriodAnnotation: Story = {
  name: 'shows an unselected annotation period in the band.',
  args: {
    bars: generateSampleBars(30),
    annotations: [periodAnnotation],
    selectedAnnotationId: null,
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}

export const SelectedPeriodWithPrice: Story = {
  name: 'shows a selected period price line across its interval.',
  args: {
    bars: generateSampleBars(30),
    annotations: [periodAnnotation],
    selectedAnnotationId: periodAnnotation.id,
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}

export const SelectedPeriodWithoutPrice: Story = {
  name: 'highlights a selected period without a price line.',
  args: {
    bars: generateSampleBars(30),
    annotations: [periodAnnotationWithoutPrice],
    selectedAnnotationId: periodAnnotationWithoutPrice.id,
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}

export const PeriodAndPointAnnotations: Story = {
  name: 'shows a point overlapping a selectable annotation period.',
  args: {
    bars: generateSampleBars(30),
    annotations: [
      periodAnnotation,
      {
        id: 'annotation-point-in-period',
        target_kind: 'sample-kind',
        status: 'approved',
        timestamp: '2025-01-07T00:00:00.000Z',
        price: null,
        text: '期間内の点注釈です。',
      },
    ],
    selectedAnnotationId: 'annotation-point-in-period',
    onSelectAnnotation: () => undefined,
    className: 'h-full w-full',
  },
}
