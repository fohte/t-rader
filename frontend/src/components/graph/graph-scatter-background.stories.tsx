import type { Meta, StoryObj } from '@storybook/react-vite'

import { GraphScatterBackgroundView } from '#components/graph/graph-scatter-background'

const meta = {
  title: 'Graph/GraphScatterBackground',
  component: GraphScatterBackgroundView,
  decorators: [
    (Story) => (
      <div className="size-140">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof GraphScatterBackgroundView>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'The scatter background divides a square into four areas.',
}
