import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'

const options: FilterOption[] = [
  { id: 'long-term', label: '長期投資', count: 12 },
  { id: 'mid-term', label: '中期投資', count: 5 },
  { id: 'swing', label: '集中スイング', count: 3 },
]

function Interactive({ initial }: { initial: string }) {
  const [value, setValue] = useState(initial)
  return (
    <FilterBar
      options={options}
      value={value}
      onChange={setValue}
      allLabel="すべて"
      allCount={20}
    />
  )
}

const meta = {
  title: 'Runs/FilterBar',
  parameters: { layout: 'padded' },
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const All: Story = {
  render: () => <Interactive initial="all" />,
}

export const OptionActive: Story = {
  render: () => <Interactive initial="mid-term" />,
}
