import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { WidgetToplist } from '#components/widgets/widget-toplist'
import type { components } from '#lib/api/schema.gen'
import type {
  WidgetDisplayDefinitions,
  WidgetQueryDefinitions,
} from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetToplistDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'toplist' }
>

const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}
const display: WidgetDisplayDefinitions = {
  change: { label: '前日比', format: 'percent' },
}
const viz: WidgetToplistDefinition = {
  type: 'toplist',
  value: 'change',
  order: 'desc',
  limit: 5,
}
const mixedResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'stock:DEMO-C',
      name: '架空銘柄 C',
      values: { change: -0.006 },
    },
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { change: 0.024 },
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
    {
      key: 'stock:DEMO-E',
      name: '架空銘柄 E',
      values: { change: 0 },
    },
  ],
}
const singleLayerViz: WidgetToplistDefinition = {
  ...viz,
  bar_scale: { min: -0.04, max: 0.04 },
}
const longNameResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'stock:DEMO-LONG-A',
      name: '架空銘柄 長い名称が狭い表示幅でも読みやすく表示されることを確認する',
      values: { change: 0.032 },
    },
    {
      key: 'stock:DEMO-LONG-B',
      name: '架空銘柄 長い名前と同じ値を持つ項目',
      values: { change: -0.012 },
    },
    {
      key: 'stock:DEMO-LONG-C',
      name: '架空銘柄 方向を比較するための例',
      values: { change: 0.004 },
    },
  ],
}
const outlierResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'stock:DEMO-OUTLIER',
      name: '架空銘柄 外れ値',
      values: { change: 0.18 },
    },
    {
      key: 'stock:DEMO-MID-A',
      name: '架空銘柄 中央値 A',
      values: { change: 0.024 },
    },
    {
      key: 'stock:DEMO-MID-B',
      name: '架空銘柄 中央値 B',
      values: { change: -0.012 },
    },
  ],
}
const zeroResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: mixedResult.rows.map((row) => ({
    ...row,
    values: { ...row.values, change: 0 },
  })),
}
const positiveResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: mixedResult.rows.map((row, index) => ({
    ...row,
    values: { ...row.values, change: (index + 1) * 0.003 },
  })),
}
const negativeResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: mixedResult.rows.map((row, index) => ({
    ...row,
    values: { ...row.values, change: -(index + 1) * 0.003 },
  })),
}
const indicatorResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'indicator:DEMO-INDEX',
      name: '架空指標',
      values: { change: 0.024 },
    },
  ],
}
const groupResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'group:demo-axis/demo-group',
      name: '架空分類',
      values: { change: -0.012 },
    },
  ],
}
const comparisonResult: WidgetQueryResponse = {
  as_of: '2099-04-12',
  rows: [
    {
      key: 'stock:DEMO-COMPARE-A',
      name: '架空銘柄 比較 A',
      values: { change: 0.032 },
    },
    {
      key: 'stock:DEMO-COMPARE-B',
      name: '架空銘柄 比較 B',
      values: { change: 0.016 },
    },
    {
      key: 'stock:DEMO-COMPARE-C',
      name: '架空銘柄 比較 C',
      values: { change: -0.008 },
    },
    {
      key: 'stock:DEMO-COMPARE-D',
      name: '架空銘柄 比較 D',
      values: { change: 0.024 },
    },
    {
      key: 'stock:DEMO-COMPARE-E',
      name: '架空銘柄 比較 E',
      values: { change: -0.016 },
    },
    {
      key: 'stock:DEMO-COMPARE-F',
      name: '架空銘柄 比較 F',
      values: { change: -0.032 },
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
        {
          paths: [
            '/groups/$axisKey/$groupKey',
            '/indicators/$indicatorId',
            '/stocks/$stockId',
          ],
        },
      )}
    />
  )
}

const meta = {
  title: 'Widgets/WidgetToplist',
  component: WidgetToplist,
  decorators: [withWidgetRouter],
  args: {
    queries,
    display,
    result: mixedResult,
    viz,
  },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof WidgetToplist>

export default meta
type Story = StoryObj<typeof meta>

export const MixedChanges: Story = {
  name: '上昇と下落を同じ 0 起点の尺度で比較する。',
}

export const AllZero: Story = {
  name: 'すべての値がゼロのときに中立の基線を表示する。',
  args: { result: zeroResult },
}

export const PositiveOnly: Story = {
  name: '正の値だけを 0 起点の尺度で表示する。',
  args: { result: positiveResult },
}

export const NegativeOnly: Story = {
  name: '負の値だけを 0 起点の尺度で表示する。',
  args: { result: negativeResult },
}

export const Outlier: Story = {
  name: '指定尺度を超える外れ値を同じ尺度の端で比較する。',
  args: { result: outlierResult, viz: singleLayerViz },
}

export const LongNames: Story = {
  name: '長い名称を省略して値とコードを読み取れる。',
  args: { result: longNameResult },
}

export const NarrowWidth: Story = {
  name: '幅 360 px で名称と値を表示する。',
  render: (args) => (
    <div className="w-90">
      <WidgetToplist {...args} />
    </div>
  ),
}

export const MediumWidth: Story = {
  name: '幅 600 px でバーの差を比較する。',
  render: (args) => (
    <div className="w-150">
      <WidgetToplist {...args} />
    </div>
  ),
}

export const WideWidth: Story = {
  name: '幅 1120 px で名称とバーをゆったり表示する。',
  render: (args) => (
    <div className="w-280">
      <WidgetToplist {...args} />
    </div>
  ),
}

export const Empty: Story = {
  name: '表示できる値がない状態を示す。',
  args: { result: { as_of: '2099-04-12', rows: [] } },
}

export const SelectedStock: Story = {
  name: '選択した銘柄の詳細とアプリ内リンクを表示する。',
  args: { initialSelectedRowKey: 'stock:DEMO-A' },
}

export const ExpandedRankList: Story = {
  name: '順位一覧を開いて各行の詳細操作を表示する。',
  args: { initialRankListExpanded: true },
}

export const SelectedIndicator: Story = {
  name: '選択した指標の詳細とアプリ内リンクを表示する。',
  args: {
    result: indicatorResult,
    initialSelectedRowKey: 'indicator:DEMO-INDEX',
  },
}

export const SelectedGroup: Story = {
  name: '選択した分類の詳細とアプリ内リンクを表示する。',
  args: {
    result: groupResult,
    initialSelectedRowKey: 'group:demo-axis/demo-group',
  },
}

export const SharedScaleComparison: Story = {
  name: '同じ基準日と尺度を使う左右のリストを比較する。',
  args: { result: comparisonResult, viz: singleLayerViz },
  render: (args) => {
    const midpoint = Math.ceil(args.result.rows.length / 2)
    const leftResult = {
      ...args.result,
      rows: args.result.rows.slice(0, midpoint),
    }
    const rightResult = {
      ...args.result,
      rows: args.result.rows.slice(midpoint),
    }
    return (
      <div className="grid w-280 grid-cols-2 gap-6">
        <WidgetToplist {...args} result={leftResult} />
        <WidgetToplist {...args} result={rightResult} />
      </div>
    )
  },
}
