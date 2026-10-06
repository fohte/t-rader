import type { Meta, StoryObj } from '@storybook/react-vite'

import { CHANGE_REFERENCE_BARS } from '#components/note-detail/change-reference-bars.fixtures'
import { ChangeReferenceFigureView } from '#components/note-detail/change-reference-figure-view'

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
  args: { state: { status: 'ready', bars: CHANGE_REFERENCE_BARS } },
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
