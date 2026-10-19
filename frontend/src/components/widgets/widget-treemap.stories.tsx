import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { WidgetTreemap } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const groupResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'group:demo-axis/demo-group-a',
      name: 'デモグループ A',
      values: { size: 48, change: 0.0042 },
    },
    {
      key: 'group:demo-axis/demo-group-b',
      name: 'デモグループ B',
      values: { size: 30, change: -0.009 },
    },
    {
      key: 'group:demo-axis/demo-group-c',
      name: 'デモグループ C',
      values: { size: 22, change: -0.00001 },
    },
  ],
}

const stockResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { size: 54, change: 0.0012 },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { size: 46, change: -0.0009 },
    },
  ],
}

const manyGroupResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: Array.from({ length: 12 }, (_, index) => {
    const suffix = String(index + 1).padStart(2, '0')
    return {
      key: `group:demo-axis/demo-group-${suffix}`,
      name: `デモグループ ${suffix}`,
      values: { size: 1, change: index % 2 === 0 ? 0.0001 : -0.0001 },
    }
  }),
}

const groupViz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'group' },
}
const stockQueries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}

function withGroupRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        {
          paths: ['/groups/$axisKey/$groupKey'],
        },
      )}
    />
  )
}

const meta = {
  title: 'Widgets/WidgetTreemap',
  component: WidgetTreemap,
  decorators: [withGroupRouter],
  args: { queries, result: groupResult, viz: groupViz },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof WidgetTreemap>

export default meta
type Story = StoryObj<typeof meta>

export const GroupRows: Story = {
  name: '小さな騰落率を符号付きパーセントで表示し、各ページへリンクする。',
}

export const StockRows: Story = {
  name: '銘柄別の小さな騰落率を符号付きパーセントで表示する。',
  args: { queries: stockQueries, result: stockResult },
}

export const ManyGroups: Story = {
  name: 'タイルが多い場合も各タイルの広さに応じてラベルを表示する。',
  args: { result: manyGroupResult },
}

export const Empty: Story = {
  name: '表示できるデータがない状態を表示する。',
  args: { result: { as_of: '2099-01-01', rows: [] } },
}
