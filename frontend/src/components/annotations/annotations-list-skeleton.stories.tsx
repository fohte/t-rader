import type { Meta, StoryObj } from '@storybook/react-vite'

import { AnnotationsListSkeleton } from '#components/annotations/annotations-list-skeleton'

const meta = {
  title: 'Annotations/AnnotationsListSkeleton',
  component: AnnotationsListSkeleton,
} satisfies Meta<typeof AnnotationsListSkeleton>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  name: 'shows the loading state for the annotation list.',
}
