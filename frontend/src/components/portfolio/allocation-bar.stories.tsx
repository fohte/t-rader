import type { Meta, StoryObj } from '@storybook/react-vite'

import { AllocationBar } from '#components/portfolio/allocation-bar'

const meta = {
  title: 'Portfolio/AllocationBar',
  component: AllocationBar,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof AllocationBar>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows market value allocation and the latest price observation date.',
  args: {
    segments: [
      { key: 'p1', label: '架空商事', value: 135000, kind: 'position' },
      { key: 'p2', label: 'サンプル工業', value: 114000, kind: 'position' },
      { key: 'p3', label: '仮想電機', value: 88000, kind: 'position' },
      { key: 'cash', label: '現金', value: 390000, kind: 'cash' },
    ],
    priceObservedAt: '2099-01-01',
  },
}

export const MissingMarketValue: Story = {
  name: 'marks holdings without a market value as unavailable.',
  args: {
    segments: [
      { key: 'p1', label: '架空商事', value: 135000, kind: 'position' },
      { key: 'p2', label: 'サンプル工業', value: null, kind: 'position' },
      { key: 'cash', label: '現金', value: 390000, kind: 'cash' },
    ],
    priceObservedAt: null,
  },
}

export const AllCash: Story = {
  name: 'shows a portfolio allocated entirely to cash.',
  args: {
    segments: [{ key: 'cash', label: '現金', value: 1000000, kind: 'cash' }],
    priceObservedAt: '2099-01-01',
  },
}

export const Empty: Story = {
  name: 'shows the allocation bar without any segments.',
  args: { segments: [] },
}
