import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChangeReferenceFigureView } from '#components/note-detail/change-reference-figure-view'
import type { components } from '#lib/api/schema.gen'

const BARS: components['schemas']['Bar'][] = [
  {
    instrument_id: 'fictional-code',
    timeframe: '1d',
    timestamp: '2030-01-02T00:00:00Z',
    open: 120,
    high: 128,
    low: 118,
    close: 125,
    volume: 1200,
  },
  {
    instrument_id: 'fictional-code',
    timeframe: '1d',
    timestamp: '2030-01-03T00:00:00Z',
    open: 125,
    high: 127,
    low: 114,
    close: 116,
    volume: 1500,
  },
]

const meta = {
  title: 'NoteDetail/ChangeReferenceFigureView',
  component: ChangeReferenceFigureView,
  args: {
    instrumentId: 'fictional-code',
    start: '2030-01-02',
    end: '2030-01-03',
  },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof ChangeReferenceFigureView>

export default meta
type Story = StoryObj<typeof meta>

export const Loaded: Story = {
  name: 'shows daily bars for the selected date range.',
  args: { state: { status: 'ready', bars: BARS } },
}

export const Loading: Story = {
  name: 'shows a loading placeholder while bars are requested.',
  args: { state: { status: 'loading' } },
}

export const Empty: Story = {
  name: 'shows a message when the date range has no bars.',
  args: { state: { status: 'empty' } },
}

export const Error: Story = {
  name: 'shows a message when bars cannot be loaded.',
  args: { state: { status: 'error' } },
}
