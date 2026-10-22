import type { Meta, StoryObj } from '@storybook/react-vite'

import { WidgetLine } from '#components/widgets/widget-line'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const queries: WidgetQueryDefinitions = {
  movement: { metric: 'change', period: '20d', by: 'group' },
}
const display = { movement: { label: '騰落率' } }
const result: WidgetQueryResponse = {
  as_of: '2099-01-19',
  rows: [],
  series: [
    {
      key: 'movement:group:demo-axis/demo-group-a',
      query: 'movement',
      name: '騰落率',
      target: {
        key: 'group:demo-axis/demo-group-a',
        name: 'サンプル分類 A',
      },
      points: [
        { date: '2099-01-05', value: 0.012 },
        { date: '2099-01-06', value: null },
        { date: '2099-01-07', value: -0.008 },
        { date: '2099-01-12', value: 0.021 },
        { date: '2099-01-19', value: 0.014 },
      ],
    },
    {
      key: 'movement:group:demo-axis/demo-group-b',
      query: 'movement',
      name: '騰落率',
      target: {
        key: 'group:demo-axis/demo-group-b',
        name: 'サンプル分類 B',
      },
      points: [
        { date: '2099-01-05', value: -0.006 },
        { date: '2099-01-06', value: 0.009 },
        { date: '2099-01-07', value: 0.015 },
        { date: '2099-01-12', value: -0.011 },
        { date: '2099-01-19', value: 0.004 },
      ],
    },
  ],
}
const viz: Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'line' }
> = { type: 'line', series: ['movement'] }

const meta = {
  title: 'Widgets/WidgetLine',
  component: WidgetLine,
  parameters: { layout: 'padded' },
  args: { queries, display, result, viz },
} satisfies Meta<typeof WidgetLine>

export default meta
type Story = StoryObj<typeof meta>

export const GroupComparison: Story = {
  name: 'グループ別の騰落率を欠損値付きで比較する。',
}
export const Empty: Story = {
  name: '値のない期間系列の空状態を表示する。',
  args: {
    result: {
      as_of: '2099-01-19',
      rows: [],
      series: [
        {
          key: 'movement:group:demo-axis/demo-group-a',
          query: 'movement',
          name: '騰落率',
          points: [
            { date: '2099-01-05', value: null },
            { date: '2099-01-19', value: null },
          ],
        },
      ],
    },
  },
}
