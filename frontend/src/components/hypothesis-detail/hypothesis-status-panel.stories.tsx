import type { Meta, StoryObj } from '@storybook/react-vite'

import { HypothesisStatusPanelView } from '#components/hypothesis-detail/hypothesis-status-panel'

const meta = {
  title: 'HypothesisDetail/HypothesisStatusPanel',
  component: HypothesisStatusPanelView,
  decorators: [
    (Story) => (
      <div className="max-w-sm bg-background p-5 font-sans text-foreground">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof HypothesisStatusPanelView>

export default meta
type Story = StoryObj<typeof meta>

const args = {
  status: 'unverified',
  onStatusChange: () => {},
  isUpdating: false,
  hasUpdateError: false,
}

export const Default: Story = { args }

export const Updating: Story = {
  args: { ...args, isUpdating: true },
}

export const UpdateError: Story = {
  args: { ...args, hasUpdateError: true },
}
