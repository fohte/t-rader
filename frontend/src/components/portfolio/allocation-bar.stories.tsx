import type { Meta, StoryObj } from '@storybook/react-vite'

import { AllocationBar } from '@/components/portfolio/allocation-bar'

const meta = {
  title: 'Portfolio/AllocationBar',
  component: AllocationBar,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof AllocationBar>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    segments: [
      { key: 'p1', label: 'SUMCO', value: 296000, kind: 'position' },
      { key: 'p2', label: 'トヨタ自動車', value: 281000, kind: 'position' },
      { key: 'p3', label: 'ソニーグループ', value: 180000, kind: 'position' },
      { key: 'cash', label: '現金', value: 1570000, kind: 'cash' },
    ],
  },
}

export const AllCash: Story = {
  args: {
    segments: [{ key: 'cash', label: '現金', value: 1000000, kind: 'cash' }],
  },
}

export const Empty: Story = {
  args: { segments: [] },
}
