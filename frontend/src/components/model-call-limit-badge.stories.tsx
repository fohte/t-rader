import type { Meta, StoryObj } from '@storybook/react-vite'

import { ModelCallLimitBadge } from '#components/model-call-limit-badge'

const meta = {
  title: 'Task/ModelCallLimitBadge',
  component: ModelCallLimitBadge,
  decorators: [
    (Story) => (
      <div className="bg-background p-4">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ModelCallLimitBadge>

export default meta
type Story = StoryObj<typeof meta>

export const StoppedAtLimit: Story = {
  name: 'marks a task stopped at the model call limit.',
}
