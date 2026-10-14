import type { Meta, StoryObj } from '@storybook/react-vite'

import { ReferenceDetailState } from '#components/reference-detail-state'

const meta = {
  title: 'References/ReferenceDetailState',
  component: ReferenceDetailState,
  args: { label: 'グループ', identifier: 'demo-axis/demo-group' },
} satisfies Meta<typeof ReferenceDetailState>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'the page shows a placeholder while a reference loads.',
  args: { state: 'loading' },
}

export const NotFound: Story = {
  name: 'the page explains when a reference does not exist.',
  args: { state: 'not-found' },
}

export const Error: Story = {
  name: 'the page explains when a reference cannot be loaded.',
  args: { state: 'error' },
}
