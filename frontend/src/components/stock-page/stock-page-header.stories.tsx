import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { StockPageHeader } from '#components/stock-page/stock-page-header'
import type { components } from '#lib/api/schema.gen'
import type { StockQuote } from '#lib/stock-page-model'
import { createStoryRouter } from '#storybook/story-router'

type Stock = components['schemas']['Stock']

const stock: Stock = {
  id: 'demo-code',
  name: 'デモ商事',
  market: 'TSE',
  sector_id: null,
  product_category: null,
  created_at: '2030-04-01T00:00:00Z',
  updated_at: '2030-04-01T00:00:00Z',
}

const quote: StockQuote = {
  close: 3250,
  change: 50,
  changePercent: 1.56,
  timestamp: '2030-04-03T06:00:00Z',
}

function withRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/groups/$axisKey/$groupKey'],
        },
      )}
    />
  )
}

const meta = {
  title: 'StockPage/StockPageHeader',
  component: StockPageHeader,
  decorators: [withRouter],
  args: {
    stock,
    quote,
    isHolding: true,
    groups: [
      {
        axis: { key: 'demo-axis', name: 'デモ軸' },
        group: { key: 'demo-group', name: 'デモ群' },
      },
    ],
    groupsState: 'ready',
  },
} satisfies Meta<typeof StockPageHeader>

export default meta
type Story = StoryObj<typeof meta>

export const WithDailyQuote: Story = {
  name: '架空銘柄の名前と直近の日足騰落を表示する。',
}

export const DailyQuoteDecline: Story = {
  name: '架空銘柄の直近の日足下落を down 色で表示する。',
  args: {
    quote: { ...quote, close: 3200, change: -50, changePercent: -1.56 },
  },
}

export const WithoutHolding: Story = {
  name: '保有していない銘柄では保有印を表示しない。',
  args: { isHolding: false },
}

export const QuoteLoading: Story = {
  name: '日足の終値を読み込んでいる状態を表示する。',
  args: { quote: null, quoteLoading: true },
}

export const NoQuote: Story = {
  name: '日足の終値がない状態を表示する。',
  args: { quote: null },
}

export const QuoteError: Story = {
  name: '日足の終値を取得できなかった状態を表示する。',
  args: { quote: null, quoteError: true },
}
