import type { Meta, StoryObj } from '@storybook/react-vite'

import { CreateHypothesisButton } from '#components/hypothesis-list/create-hypothesis-button'

const meta = {
  title: 'HypothesisList/CreateHypothesisButton',
  component: CreateHypothesisButton,
} satisfies Meta<typeof CreateHypothesisButton>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows the action for creating a new hypothesis.',
  args: { onClick: () => {} },
}
