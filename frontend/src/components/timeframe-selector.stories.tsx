import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'
import { useState } from 'react'

import {
  TimeframeSelector,
  type Timeframe,
} from '@/components/timeframe-selector'

const meta = {
  title: 'Components/TimeframeSelector',
  component: TimeframeSelector,
  args: {
    value: '1d',
    onChange: fn(),
  },
} satisfies Meta<typeof TimeframeSelector>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}

export const Interactive: Story = {
  render: () => {
    const [value, setValue] = useState<Timeframe>('1d')
    return <TimeframeSelector value={value} onChange={setValue} />
  },
}
