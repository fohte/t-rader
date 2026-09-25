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

export const Default: Story = {
  name: 'shows an unverified hypothesis and its status controls.',
  args,
}

export const Updating: Story = {
  name: 'shows the status panel while a change is being saved.',
  args: { ...args, isUpdating: true },
}

export const UpdateError: Story = {
  name: 'shows the status panel after a status change fails.',
  args: { ...args, hasUpdateError: true },
}
