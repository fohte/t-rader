import type { Meta, StoryObj } from '@storybook/react-vite'

import { HypothesisListSkeleton } from '#components/hypothesis-list/hypothesis-list-skeleton'

const meta = {
  title: 'HypothesisList/HypothesisListSkeleton',
  component: HypothesisListSkeleton,
} satisfies Meta<typeof HypothesisListSkeleton>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows placeholder rows while the hypothesis list is loading.',
}
