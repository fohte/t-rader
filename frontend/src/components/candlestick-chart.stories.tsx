import type { Meta, StoryObj } from '@storybook/react-vite'

import { CandlestickChart } from '#components/candlestick-chart'
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
      instrument_id: '7203',
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

const meta = {
  title: 'Components/CandlestickChart',
  component: CandlestickChart,
  decorators: [
    (Story) => (
      <div style={{ width: '100%', height: '600px' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CandlestickChart>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    bars: generateSampleBars(120),
    className: 'h-full w-full',
  },
}

export const FewBars: Story = {
  args: {
    bars: generateSampleBars(10),
    className: 'h-full w-full',
  },
}

export const Empty: Story = {
  args: {
    bars: [],
    className: 'h-full w-full',
  },
}
