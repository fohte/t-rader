import type { Meta, StoryObj } from '@storybook/react-vite'

import { CandlestickChart } from '@/components/candlestick-chart'
import type { components } from '@/lib/api/schema.gen'

type Bar = components['schemas']['Bar']

/** サンプルデータを生成する */
function generateSampleBars(count: number): Bar[] {
  const bars: Bar[] = []
  let price = 1500

  for (let i = 0; i < count; i++) {
    const date = new Date(2025, 0, 1)
    date.setDate(date.getDate() + i)

    const open = price + (Math.random() - 0.5) * 50
    const close = open + (Math.random() - 0.5) * 60
    const high = Math.max(open, close) + Math.random() * 30
    const low = Math.min(open, close) - Math.random() * 30
    const volume = Math.floor(100000 + Math.random() * 500000)

    bars.push({
      instrument_id: '7203',
      timeframe: '1d',
      timestamp: date.toISOString(),
      open: open.toFixed(1),
      high: high.toFixed(1),
      low: low.toFixed(1),
      close: close.toFixed(1),
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
