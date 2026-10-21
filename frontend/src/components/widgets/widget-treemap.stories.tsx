import type { Meta, StoryObj } from '@storybook/react-vite'
import { RouterProvider } from '@tanstack/react-router'
import type { ComponentType } from 'react'

import { WidgetTreemap } from '#components/widgets/widget-treemap'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const hierarchicalResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: Array.from({ length: 33 }, (_, groupIndex) => {
    const groupNumber = String(groupIndex + 1).padStart(2, '0')
    const sizes =
      groupIndex === 0
        ? [720, 120, 0.01]
        : groupIndex === 1
          ? [170, 48]
          : [40 / (groupIndex + 1)]
    return sizes.map((size, rowIndex) => {
      const rowNumber = String(groupIndex * 3 + rowIndex + 1).padStart(3, '0')
      const values: Record<string, number | null> = { size }
      if (groupIndex === 30) values.change = null
      else if (groupIndex !== 31) {
        values.change =
          groupIndex === 32
            ? 0
            : (groupIndex % 2 === 0 ? 1 : -1) * (0.001 + groupIndex * 0.0001)
      }

      return {
        key: `stock:DEMO-${rowNumber}`,
        name:
          groupIndex === 0 && rowIndex === 0
            ? '架空銘柄 001 名称が長い場合にも値を読み取れる表示を確認するサンプル'
            : `架空銘柄 ${rowNumber}`,
        values,
        classification: {
          axis: 'demo-axis',
          group: {
            key: `demo-group-${groupNumber}`,
            name:
              groupIndex === 0
                ? '架空分類 01 とても長い名前を省略表示するサンプル'
                : `架空分類 ${groupNumber}`,
          },
        },
      }
    })
  }).flat(),
}

const singleLayerResult: WidgetQueryResponse = {
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

const groupResult: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'group:demo-axis/demo-group-a',
      name: '架空分類 A',
      values: { size: 48, change: 0.0042 },
    },
    {
      key: 'group:demo-axis/demo-group-b',
      name: '架空分類 B',
      values: { size: 30, change: -0.009 },
    },
    {
      key: 'group:demo-axis/demo-group-c',
      name: '架空分類 C',
      values: { size: 22, change: -0.00001 },
    },
  ],
}

const hierarchyViz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
  hierarchy: { axis: 'demo-axis' },
}
const singleLayerViz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}

function withWidgetRouter(Story: ComponentType) {
  return (
    <RouterProvider
      router={createStoryRouter(
        () => (
          <Story />
        ),
        { paths: ['/groups/$axisKey/$groupKey', '/stocks/$stockId'] },
      )}
    />
  )
}

const meta = {
  title: 'Widgets/WidgetTreemap',
  component: WidgetTreemap,
  decorators: [withWidgetRouter],
  args: { queries, result: hierarchicalResult, viz: hierarchyViz },
  parameters: { layout: 'padded' },
} satisfies Meta<typeof WidgetTreemap>

export default meta
type Story = StoryObj<typeof meta>

export const HierarchicalOverview: Story = {
  name: '33 分類と配下銘柄を面積と騰落の濃淡で比較する。',
}

export const HierarchicalFocus: Story = {
  name: '分類へフォーカスすると配下銘柄を広く表示する。',
  args: { initialFocusGroupKey: 'demo-group-01' },
}

export const SelectedStock: Story = {
  name: '選択した銘柄の詳細とアプリ内リンクを表示する。',
  args: { initialSelectedRowKey: 'stock:DEMO-001' },
}

export const ExpandedList: Story = {
  name: '全件一覧を開いて銘柄と分類の操作を表示する。',
  args: { initialListExpanded: true },
}

export const NarrowWidth: Story = {
  name: '幅 360 px でも分類名と値を収めて表示する。',
  render: (args) => (
    <div className="w-90">
      <WidgetTreemap {...args} />
    </div>
  ),
}

export const MediumWidth: Story = {
  name: '幅 600 px で分類名と値を並べて表示する。',
  render: (args) => (
    <div className="w-150">
      <WidgetTreemap {...args} />
    </div>
  ),
}

export const WideWidth: Story = {
  name: '幅 1120 px で全体の分類を表示する。',
  render: (args) => (
    <div className="w-280">
      <WidgetTreemap {...args} />
    </div>
  ),
}

export const SingleLayer: Story = {
  name: '階層指定がない場合に従来の単層表示を使う。',
  args: { result: singleLayerResult, viz: singleLayerViz },
}

export const FormulaPercent: Story = {
  name: '明示した表示形式で式の値をパーセント表示する。',
  args: {
    queries: {},
    display: {
      relative: { label: '架空相対値', format: 'percent' },
    },
    result: {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'stock:DEMO-PERCENT',
          name: '架空銘柄 パーセント',
          values: { size: 1, relative: 0.0123 },
        },
      ],
    },
    viz: { type: 'treemap', size: 'size', color: 'relative' },
  },
}

export const GroupRows: Story = {
  name: '選択した分類の詳細と分類ページへのリンクを表示する。',
  args: {
    result: groupResult,
    viz: singleLayerViz,
    initialSelectedRowKey: 'group:demo-axis/demo-group-a',
  },
}

export const AllZero: Story = {
  name: '騰落がすべてゼロの場合に中立色で表示する。',
  args: {
    result: {
      as_of: '2099-01-01',
      rows: hierarchicalResult.rows.slice(0, 3).map((row) => ({
        ...row,
        values: { ...row.values, change: 0 },
      })),
    },
  },
}

export const PositiveOnly: Story = {
  name: '上昇値だけのデータで同じ色尺度を使う。',
  args: {
    result: {
      as_of: '2099-01-01',
      rows: hierarchicalResult.rows.slice(0, 3).map((row) => ({
        ...row,
        values: { ...row.values, change: 0.012 },
      })),
    },
  },
}

export const NegativeOnly: Story = {
  name: '下落値だけのデータで同じ色尺度を使う。',
  args: {
    result: {
      as_of: '2099-01-01',
      rows: hierarchicalResult.rows.slice(0, 3).map((row) => ({
        ...row,
        values: { ...row.values, change: -0.012 },
      })),
    },
  },
}

export const MissingValues: Story = {
  name: '値が欠損した項目も全件一覧から確認できる。',
  args: {
    initialListExpanded: true,
    result: {
      as_of: '2099-01-01',
      rows: [
        ...hierarchicalResult.rows.slice(0, 2),
        {
          key: 'stock:DEMO-MISSING',
          name: '架空銘柄 値欠損',
          values: { change: null },
          classification: {
            axis: 'demo-axis',
            group: { key: 'demo-group-01', name: '架空分類 01' },
          },
        },
      ],
    },
  },
}

export const Empty: Story = {
  name: '表示できる項目がない状態を表示する。',
  args: { result: { as_of: '2099-01-01', rows: [] } },
}
