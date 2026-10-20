import type { Meta, StoryObj } from '@storybook/react-vite'

import { WidgetToplist } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'stock:DEMO-C',
      name: '架空銘柄 C',
      values: { change: -0.006 },
    },
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { change: 0.021 },
    },
    {
      key: 'stock:DEMO-D',
      name: '架空銘柄 D',
      values: { change: -0.014 },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { change: 0.008 },
    },
  ],
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}

const meta = {
  title: 'Widgets/WidgetToplist',
  component: WidgetToplist,
  args: {
    queries,
    result,
    viz: { type: 'toplist', value: 'change', order: 'desc', limit: 3 },
  },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof WidgetToplist>

export default meta
type Story = StoryObj<typeof meta>

export const Descending: Story = {
  name: '小さな騰落率を大きい順に符号付きパーセントで順位表示する。',
}

export const Ascending: Story = {
  name: '値の小さい順に指定件数の行を表示する。',
  args: {
    viz: { type: 'toplist', value: 'change', order: 'asc', limit: 2 },
  },
}

export const FormulaPercent: Story = {
  name: '明示した表示形式で式の値をパーセント表示する。',
  args: {
    queries: {},
    display: { relative: { label: 'Relative', format: 'percent' } },
    result: {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'stock:DEMO-A',
          name: '架空銘柄 A',
          values: { relative: 0.0123 },
        },
      ],
    },
    viz: { type: 'toplist', value: 'relative', order: 'desc', limit: 1 },
  },
}

export const Empty: Story = {
  name: '表示できるデータがない状態を表示する。',
  args: {
    result: { as_of: '2099-01-01', rows: [] },
  },
}
