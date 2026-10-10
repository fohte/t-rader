import type { Meta, StoryObj } from '@storybook/react-vite'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'

const markers = [
  { id: 'annotation-a', x: 120 },
  { id: 'annotation-b', x: 126 },
  { id: 'annotation-c', x: 310 },
  { id: 'annotation-d', x: 520 },
]

const meta = {
  title: 'Annotations/AnnotationChartBand',
  component: AnnotationChartBand,
  args: {
    markers,
    width: 640,
    bottom: 28,
    selectedAnnotationId: 'annotation-b',
    onSelectAnnotation: () => undefined,
  },
  decorators: [
    (Story) => (
      <div className="relative h-24 w-170 border border-border bg-background">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AnnotationChartBand>

export default meta
type Story = StoryObj<typeof meta>

export const WithNearbyAnnotations: Story = {
  name: 'shows nearby annotations as a selected count.',
}

export const WithoutSelection: Story = {
  name: 'shows the annotation band before an annotation is selected.',
  args: { selectedAnnotationId: null },
}
