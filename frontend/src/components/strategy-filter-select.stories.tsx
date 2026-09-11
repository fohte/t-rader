import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { StrategyFilterSelectView } from '#components/strategy-filter-select'

const meta = {
  title: 'StrategyFilterSelect',
  component: StrategyFilterSelectView,
  parameters: { layout: 'centered' },
  args: { onChange: fn() },
} satisfies Meta<typeof StrategyFilterSelectView>

export default meta
type Story = StoryObj<typeof meta>

const strategy1 = {
  id: '00000000-0000-0000-0000-000000000001',
  name: '長期投資',
  description: null,
  sort_order: 0,
  risk_policy: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}
const strategy2 = {
  id: '00000000-0000-0000-0000-000000000002',
  name: '集中スイング',
  description: null,
  sort_order: 1,
  risk_policy: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}
const strategies = [strategy1, strategy2]

export const AllStrategies: Story = {
  args: { value: undefined, strategies },
}

export const StrategySelected: Story = {
  args: { value: strategy1.id, strategies },
}
