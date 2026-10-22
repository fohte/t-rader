import type { Meta, StoryObj } from '@storybook/react-vite'

import { Skeleton } from '#components/ui/skeleton'

const meta = {
  title: 'UI/Skeleton',
  component: Skeleton,
} satisfies Meta<typeof Skeleton>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows a horizontal loading placeholder.',
  args: {
    className: 'h-4 w-48',
  },
}

export const Square: Story = {
  name: 'shows a square loading placeholder at avatar size.',
  args: {
    className: 'size-12',
  },
}

export const Card: Story = {
  name: 'shows square avatar and text placeholders in a loading card.',
  render: () => (
    <div className="flex items-center gap-4">
      <Skeleton className="size-12" />
      <div className="space-y-2">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-4 w-32" />
      </div>
    </div>
  ),
}
