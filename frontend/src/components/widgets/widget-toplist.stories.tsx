import type { Meta, StoryObj } from '@storybook/react-vite'

import { WidgetToplist } from '#components/widgets/widget-toplist'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'stock:DEMO-C',
      name: '架空銘柄 C',
      values: { change: -0.6 },
    },
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { change: 2.1 },
    },
    {
      key: 'stock:DEMO-D',
      name: '架空銘柄 D',
      values: { change: -1.4 },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { change: 0.8 },
    },
  ],
}

const meta = {
  title: 'Widgets/WidgetToplist',
  component: WidgetToplist,
  args: {
    result,
    viz: { type: 'toplist', value: 'change', order: 'desc', limit: 3 },
  },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof WidgetToplist>

export default meta
type Story = StoryObj<typeof meta>

export const Descending: Story = {
  name: '値の大きい順に指定件数の行を表示する。',
}

export const Ascending: Story = {
  name: '値の小さい順に指定件数の行を表示する。',
  args: {
    viz: { type: 'toplist', value: 'change', order: 'asc', limit: 2 },
  },
}

export const Empty: Story = {
  name: '表示できるデータがない状態を表示する。',
  args: {
    result: { as_of: '2099-01-01', rows: [] },
  },
}
