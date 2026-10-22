import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'

const options: FilterOption[] = [
  { id: 'sample-strategy-a', label: '架空戦略 A', count: 12 },
  { id: 'sample-strategy-b', label: '架空戦略 B', count: 5 },
  { id: 'sample-strategy-c', label: '架空戦略 C', count: 3 },
]

const meta = {
  title: 'Runs/FilterBar',
  component: FilterBar,
  parameters: { layout: 'padded' },
  args: {
    options,
    value: 'all',
    onChange: fn(),
    allLabel: 'すべて',
    allCount: 20,
  },
} satisfies Meta<typeof FilterBar>

export default meta
type Story = StoryObj<typeof meta>

export const All: Story = {
  name: 'shows run counts with every strategy included.',
}

export const OptionActive: Story = {
  name: 'shows the run filter with a single strategy selected.',
  args: {
    value: 'sample-strategy-b',
  },
}
