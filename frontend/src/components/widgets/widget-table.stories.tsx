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
  pbr: { metric: 'pbr', by: 'stock' },
  close: { metric: 'close', by: 'stock' },
  turnover: { metric: 'turnover', by: 'stock' },
  per: { metric: 'per', by: 'stock' },
  indicator_value: {
    metric: 'indicator',
    by: 'none',
    indicator: 'demo-indicator',
  },
}

const display = {
  pbr: { label: 'PBR', format: 'number' },
  close: { label: '終値', format: 'number' },
  turnover: { label: '売買代金', format: 'number' },
  per: { label: 'PER', format: 'number' },
  indicator_value: { label: '指標', format: 'number' },
} satisfies components['schemas']['WidgetQueryRequest']['display']

const viz: WidgetTableDefinition = {
  type: 'table',
  columns: ['pbr', 'close', 'turnover', 'per', 'indicator_value'],
}

const result: WidgetQueryResponse = {
  as_of: '2099-01-05',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: {
        close: 1240,
        turnover: 2400000,
        per: 12.5,
        pbr: 1.3,
        indicator_value: 4.2,
      },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: {
        close: null,
        turnover: null,
        per: 18.2,
        pbr: null,
        indicator_value: null,
      },
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
      values: { close: 100 + index },
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
  args: { result: paginatedResult, viz: { type: 'table', columns: ['close'] } },
}
