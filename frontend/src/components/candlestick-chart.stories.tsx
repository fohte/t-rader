import type { Meta, StoryObj } from '@storybook/react-vite'

import { CandlestickChart } from '#components/candlestick-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']

/** [0, 1) の疑似乱数を返す。スクリーンショットの決定性のため Math.random() の代わりに使う */
function createRandom(seed: number): () => number {
  let state = seed
  return () => {
    state = (state * 1103515245 + 12345) & 0x7fffffff
    return state / 0x7fffffff
  }
}

/** サンプルデータを生成する */
function generateSampleBars(count: number): Bar[] {
  const random = createRandom(1)
  const bars: Bar[] = []
  let price = 1500

  for (let i = 0; i < count; i++) {
    const date = new Date(2025, 0, 1)
    date.setDate(date.getDate() + i)

    const open = price + (random() - 0.5) * 50
    const close = open + (random() - 0.5) * 60
    const high = Math.max(open, close) + random() * 30
    const low = Math.min(open, close) - random() * 30
    const volume = Math.floor(100000 + random() * 500000)

    bars.push({
      instrument_id: 'DEMO-JP-ALPHA',
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
      instrument_id: 'DEMO-US-ALPHA',
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

export default meta
type Story = StoryObj<typeof meta>

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
