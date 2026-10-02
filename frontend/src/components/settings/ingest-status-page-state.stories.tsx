import type { Meta, StoryObj } from '@storybook/react-vite'

import { IngestStatusPageState } from '#components/settings/ingest-status-page-state'

const meta = {
  title: 'Settings/IngestStatusPageState',
  component: IngestStatusPageState,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof IngestStatusPageState>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows placeholders while ingest status is loading.',
  args: { state: 'loading' },
}

export const Error: Story = {
  name: 'shows a message when ingest status could not be loaded.',
  args: { state: 'error' },
}
