import type { Meta, StoryObj } from '@storybook/react-vite'

import { WidgetTable } from '#components/widgets/widget-table'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTableDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'table' }
>

const queries: WidgetQueryDefinitions = {
  metric_a: { metric: 'close', by: 'stock' },
  metric_b: { metric: 'mkt_cap', by: 'stock' },
}

const display = {
  metric_a: { label: '指標 A', format: 'number' },
  metric_b: { label: '指標 B', format: 'number' },
} satisfies components['schemas']['WidgetQueryRequest']['display']

const viz: WidgetTableDefinition = {
  type: 'table',
  columns: ['metric_b', 'metric_a'],
}

const result: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: 'サンプル対象 A',
      values: { metric_a: 1240, metric_b: 24 },
    },
    {
      key: 'stock:DEMO-B',
      name: 'サンプル対象 B',
      values: { metric_a: null, metric_b: 18 },
    },
  ],
}

const paginatedResult: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: Array.from({ length: 23 }, (_, index) => {
    const number = String(index + 1).padStart(2, '0')
    return {
      key: 'stock:SAMPLE-' + number,
      name: '架空銘柄 ' + number,
      values: { metric_a: 100 + index },
    }
  }),
}

const meta = {
  title: 'Widgets/WidgetTable',
  component: WidgetTable,
  parameters: { layout: 'padded' },
  args: { result, viz, queries, display },
} satisfies Meta<typeof WidgetTable>

export default meta
type Story = StoryObj<typeof meta>

export const Populated: Story = {
  name: '指定した列順で対象名と指標の値を表示する。',
}

export const Empty: Story = {
  name: '表示できる値がない状態を示す。',
  args: { result: { as_of: '2099-01-05', rows: [] } },
}

export const Paginated: Story = {
  name: '行数が多い表で次ページへ進めることを示す。',
  args: {
    result: paginatedResult,
    viz: { type: 'table', columns: ['metric_a'] },
  },
}
