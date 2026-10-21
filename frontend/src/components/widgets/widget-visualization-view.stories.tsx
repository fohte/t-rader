import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { WidgetVisualizationView } from '#components/widgets/widget-visualization-view'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const movementQueries: WidgetQueryDefinitions = {
  movement: { metric: 'change', period: '1d', by: 'group' },
  size: { metric: 'mkt_cap', by: 'group' },
}
const movementResult: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'group:demo-axis/demo-group-a',
      name: 'サンプル分類 A',
      values: { movement: 0.0042, size: 40 },
    },
    {
      key: 'group:demo-axis/demo-group-b',
      name: 'サンプル分類 B',
      values: { movement: -0.009, size: 25 },
    },
  ],
}
const tableQueries: WidgetQueryDefinitions = {
  metric_a: { metric: 'close', by: 'stock' },
  metric_b: { metric: 'mkt_cap', by: 'stock' },
}
const tableResult: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: 'サンプル対象 A',
      values: { metric_a: 12.4, metric_b: 56.7 },
    },
    {
      key: 'stock:DEMO-B',
      name: 'サンプル対象 B',
      values: { metric_a: null, metric_b: 48.2 },
    },
  ],
}

function withWidgetRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/groups/$axisKey/$groupKey'] },
      )}
    />
  )
}

const meta = {
  title: 'Widgets/WidgetVisualizationView',
  component: WidgetVisualizationView,
  decorators: [withWidgetRouter],
  parameters: { layout: 'padded' },
  args: { queries: movementQueries, result: movementResult },
} satisfies Meta<typeof WidgetVisualizationView>

export default meta
type Story = StoryObj<typeof meta>

export const Treemap: Story = {
  name: '分類ごとの値を treemap で表示する。',
  args: { viz: { type: 'treemap', size: 'size', color: 'movement' } },
}

export const Toplist: Story = {
  name: '分類ごとの値を順位付きで表示する。',
  args: {
    viz: { type: 'toplist', value: 'movement', order: 'desc', limit: 5 },
  },
}

export const Table: Story = {
  name: '表で指定した列順に値を表示する。',
  args: {
    queries: tableQueries,
    result: tableResult,
    display: {
      metric_a: { label: '指標 A', format: 'number' },
      metric_b: { label: '指標 B', format: 'number' },
    },
    viz: { type: 'table', columns: ['metric_b', 'metric_a'] },
  },
}
