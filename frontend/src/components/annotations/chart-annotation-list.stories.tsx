import type { Meta, StoryObj } from '@storybook/react-vite'

import { ChartAnnotationList } from '#components/annotations/chart-annotation-list'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'

const periodAnnotation: ChartAnnotation = {
  id: 'annotation-b',
  created_by_kind: 'llm',
  target_kind: 'sample-kind',
  timestamp_start: '2025-01-02T00:00:00.000Z',
  timestamp: '2025-01-04T00:00:00.000Z',
  price: null,
  price_start: null,
  text: '価格線を持たないサンプル注釈です。',
  status: 'approved',
}

const humanPeriodAnnotation: ChartAnnotation = {
  ...periodAnnotation,
  id: 'annotation-c',
  created_by_kind: 'human',
  text: '架空の期間注釈です。',
}

const annotations: ChartAnnotation[] = [
  {
    id: 'annotation-a',
    created_by_kind: 'human',
    target_kind: 'sample-kind',
    timestamp: '2025-01-03T00:00:00.000Z',
    price: 1250,
    price_start: null,
    timestamp_start: null,
    text: '値動きの節目を示すサンプル注釈です。',
    status: 'approved',
  },
  periodAnnotation,
]

const meta = {
  title: 'Annotations/ChartAnnotationList',
  component: ChartAnnotationList,
  args: {
    annotations,
    selectedAnnotationId: 'annotation-a',
    onSelectAnnotation: () => undefined,
    onDeleteAnnotation: () => undefined,
  },
  decorators: [
    (Story) => (
      <div className="h-150">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChartAnnotationList>

export default meta
type Story = StoryObj<typeof meta>

export const WithSelectedAnnotation: Story = {
  name: 'shows the selected annotation with its kind and price.',
}

export const WithPeriodAnnotation: Story = {
  name: 'shows the start and end date for a period annotation.',
  args: {
    annotations: [periodAnnotation],
    selectedAnnotationId: 'annotation-b',
  },
}

export const WithDeletableHumanPeriodAnnotation: Story = {
  name: 'shows deletion for a selected human period annotation.',
  args: {
    annotations: [humanPeriodAnnotation],
    selectedAnnotationId: 'annotation-c',
  },
}

export const Empty: Story = {
  name: 'shows the empty state when there are no annotations.',
  args: { annotations: [], selectedAnnotationId: null },
}

export const Loading: Story = {
  name: 'shows a loading state while annotations are fetched.',
  args: { isLoading: true },
}

export const DeletingSelectedLine: Story = {
  name: 'disables deletion while the selected horizontal line is being deleted.',
  args: { isDeleting: true },
}

export const DeleteFailed: Story = {
  name: 'shows an error when deleting the selected horizontal line fails.',
  args: { deleteError: '削除に失敗しました' },
}
