import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'

import { TimeframeSelector } from '#components/timeframe-selector'

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

export const Default: Story = {
  name: 'shows the daily interval as the selected timeframe.',
}

export const OneMinute: Story = {
  name: 'shows the one minute interval as the selected timeframe.',
  args: {
    value: '1m',
  },
}

export const Compact: Story = {
  name: 'the compact selector shows 1h and 1D while keeping the remaining intervals in a menu.',
  args: {
    variant: 'compact',
  },
}

export const CompactMenuOpen: Story = {
  name: 'the open compact menu shows the remaining intervals.',
  args: {
    variant: 'compact',
    value: '5m',
    moreTimeframesDefaultOpen: true,
  },
}
