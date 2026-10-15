import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'

import { ScreenerStockCard } from '#components/screener/screener-stock-card'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type ScreenerStock = components['schemas']['ScreenerStockResponse']

const stock: ScreenerStock = {
  id: 'demo-code',
  name: '架空商事',
  market: 'プライム',
  sector: 'サンプル業',
  close: 3250,
  per: 8.2,
  pbr: 0.7,
  roe: 9.1,
  dividend_yield: 3.8,
  market_cap: 5140,
}

const meta = {
  title: 'Screener/ScreenerStockCard',
  component: ScreenerStockCard,
  parameters: { layout: 'padded' },
  render: (args) => (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <div className="mx-auto w-full max-w-100">
            <ScreenerStockCard {...args} />
          </div>
        ),
        { paths: ['/stocks/$stockId'] },
      )}
    />
  ),
} satisfies Meta<typeof ScreenerStockCard>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'スマートフォン幅で主要指標と銘柄コードを表示します。',
  args: { stock },
}

export const MissingMetrics: Story = {
  name: '指標が未取得のときはダッシュを表示します。',
  args: {
    stock: {
      ...stock,
      close: null,
      per: null,
      pbr: null,
      dividend_yield: null,
    },
  },
}
