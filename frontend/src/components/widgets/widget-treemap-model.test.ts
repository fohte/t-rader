import { describe, expect, it } from 'vitest'

import {
  createWidgetTreemapModel,
  createWidgetTreemapOption,
  getTreemapFillColor,
  getTreemapTextColor,
} from '#components/widgets/widget-treemap-model'
import type { components } from '#lib/api/schema.gen'
import type { EChartsPalette } from '#lib/echarts/use-echarts-chart'
import type {
  WidgetDisplayDefinitions,
  WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const palette: EChartsPalette = {
  background: '#fafafa',
  border: '#e5e5e5',
  foreground: '#0a0a0a',
  neutral: '#1a1a1a',
  up: '#ef5350',
  upDim: '#5a2a2a',
  down: '#3f9fe0',
  downDim: '#234152',
  seriesColors: ['#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de'],
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}
const display: WidgetDisplayDefinitions = {}
const viz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
  hierarchy: { axis: 'demo-axis' },
  color_scale: { min: -0.05, max: 0.04 },
}
const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    {
      key: 'stock:DEMO-A',
      name: '架空銘柄 A',
      values: { size: 9, change: 0.015 },
      classification: {
        axis: 'demo-axis',
        group: { key: 'demo-group-a', name: '架空分類 A' },
      },
    },
    {
      key: 'stock:DEMO-B',
      name: '架空銘柄 B',
      values: { size: 1, change: null },
      classification: {
        axis: 'demo-axis',
        group: { key: 'demo-group-a', name: '架空分類 A' },
      },
    },
    {
      key: 'stock:DEMO-C',
      name: '架空銘柄 C',
      values: { size: 5, change: -0.02 },
      classification: { axis: 'demo-axis', group: null },
    },
    {
      key: 'stock:DEMO-D',
      name: '架空銘柄 D',
      values: { size: 0, change: 0 },
      classification: { axis: 'demo-axis', group: null },
    },
  ],
}

describe('getTreemapTextColor', () => {
  it('データ色に対する文字色を ECharts palette の明暗から選ぶ', () => {
    const darkThemePalette: EChartsPalette = {
      ...palette,
      background: '#0a0a0a',
      foreground: '#fafafa',
    }

    function readTextColorOutput() {
      return {
        lightTheme: [
          getTreemapTextColor('#1a1a1a', palette),
          getTreemapTextColor('#fafafa', palette),
        ],
        darkTheme: [
          getTreemapTextColor('#1a1a1a', darkThemePalette),
          getTreemapTextColor('#fafafa', darkThemePalette),
        ],
      }
    }

    expect(readTextColorOutput()).toEqual({
      lightTheme: ['#fafafa', '#0a0a0a'],
      darkTheme: ['#fafafa', '#0a0a0a'],
    })
  })
})

function makeModel(input = result) {
  return createWidgetTreemapModel({ display, queries, result: input, viz })
}

describe('widget treemap model', () => {
  it('分類の面積・行順・欠損値・安定 data index を保持する', () => {
    const model = makeModel()

    function readModelOutput() {
      return {
        rows: model.rows.map((row) => ({
          key: row.key,
          area: row.area,
          formattedSize: row.formattedSize,
          formattedColor: row.formattedColor,
        })),
        groups: model.groups.map((group) => ({
          id: group.id,
          name: group.name,
          area: group.area,
          rowKeys: group.rows.map((row) => row.key),
        })),
        chartData: model.chartData.map((group) => ({
          id: group.id,
          value: group.value,
          children: group.children?.map((row) => ({
            id: row.id,
            value: row.value,
          })),
        })),
        dataIndexToRowKey: [...model.dataIndexToRowKey],
        colorScale: model.colorScale,
        labels: [model.sizeLabel, model.colorLabel, model.hierarchyAxis],
      }
    }
    expect(readModelOutput()).toEqual({
      rows: [
        {
          key: 'stock:DEMO-A',
          area: 9,
          formattedSize: '9',
          formattedColor: '+1.50%',
        },
        {
          key: 'stock:DEMO-C',
          area: 5,
          formattedSize: '5',
          formattedColor: '-2.00%',
        },
        {
          key: 'stock:DEMO-B',
          area: 1,
          formattedSize: '1',
          formattedColor: '—',
        },
        {
          key: 'stock:DEMO-D',
          area: 0,
          formattedSize: '0',
          formattedColor: '0.00%',
        },
      ],
      groups: [
        {
          id: 'sector:["demo-axis","demo-group-a"]',
          name: '架空分類 A',
          area: 10,
          rowKeys: ['stock:DEMO-A', 'stock:DEMO-B'],
        },
        {
          id: 'sector:["demo-axis",null]',
          name: '未分類',
          area: 5,
          rowKeys: ['stock:DEMO-C', 'stock:DEMO-D'],
        },
      ],
      chartData: [
        {
          id: 'sector:["demo-axis","demo-group-a"]',
          value: 10,
          children: [
            { id: 'row:stock:DEMO-A', value: 9 },
            { id: 'row:stock:DEMO-B', value: 1 },
          ],
        },
        {
          id: 'sector:["demo-axis",null]',
          value: 5,
          children: [{ id: 'row:stock:DEMO-C', value: 5 }],
        },
      ],
      dataIndexToRowKey: [
        [2, 'stock:DEMO-A'],
        [3, 'stock:DEMO-B'],
        [5, 'stock:DEMO-C'],
      ],
      colorScale: { min: -0.05, max: 0.04 },
      labels: ['size', 'change', 'demo-axis'],
    })
  })

  it('全体の明示尺度に対して正負・ゼロ・欠損を同じ配色で描く', () => {
    const scale = { min: -0.04, max: 0.04 }

    expect(
      [0.04, 0.02, 0, -0.02, -0.04, null].map((value) =>
        getTreemapFillColor(value, scale, palette),
      ),
    ).toEqual([
      '#ef5350',
      '#7c3333',
      '#1a1a1a',
      '#295773',
      '#3f9fe0',
      '#1a1a1a',
    ])
  })

  it('display の表示名と明示パーセント形式を使う', () => {
    const model = createWidgetTreemapModel({
      display: {
        relative: { label: '架空相対値', format: 'percent' },
      },
      queries: {},
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
    })

    function readDisplayOutput() {
      return {
        label: model.colorLabel,
        formattedValue: model.rows[0]?.formattedColor,
      }
    }

    expect(readDisplayOutput()).toEqual({
      label: '架空相対値',
      formattedValue: '+1.23%',
    })
  })

  it('階層データを最初から2段で描き、色尺度を全行から決める', () => {
    const model = makeModel()
    const option = createWidgetTreemapOption(model, palette, () => undefined)
    const series = Array.isArray(option.series)
      ? option.series[0]
      : option.series

    function readOptionOutput() {
      return {
        leafDepth: series?.leafDepth ?? null,
        nodeClick: series?.nodeClick,
        seriesLabel: series?.label?.show,
        labelFormatter: series?.label?.formatter,
        rootUpperLabel: series?.levels?.[0]?.upperLabel?.show ?? null,
        groupLabel: series?.levels?.[1]?.label?.show ?? null,
        groupUpperLabel: series?.levels?.[1]?.upperLabel?.show ?? null,
        groupUpperLabelFormatter:
          series?.levels?.[1]?.upperLabel?.formatter ?? null,
        levels: series?.levels?.length,
        nodes: series?.data?.map((node) => ({
          id: node.id,
          children: node.children?.map((child) => child.id),
        })),
        colorScale: model.colorScale,
      }
    }
    expect(readOptionOutput()).toEqual({
      leafDepth: null,
      nodeClick: false,
      seriesLabel: true,
      labelFormatter: '{b}',
      rootUpperLabel: null,
      groupLabel: false,
      groupUpperLabel: true,
      groupUpperLabelFormatter: '{b}',
      levels: 3,
      nodes: [
        {
          id: 'sector:["demo-axis","demo-group-a"]',
          children: ['row:stock:DEMO-A', 'row:stock:DEMO-B'],
        },
        {
          id: 'sector:["demo-axis",null]',
          children: ['row:stock:DEMO-C'],
        },
      ],
      colorScale: { min: -0.05, max: 0.04 },
    })
  })

  it('単層 treemap の行名ラベルを有効にする', () => {
    const option = createWidgetTreemapOption(
      createWidgetTreemapModel({
        display,
        queries,
        result: {
          as_of: '2099-01-01',
          rows: [
            {
              key: 'stock:DEMO-A',
              name: '架空銘柄 A',
              values: { size: 9, change: 0.015 },
            },
          ],
        },
        viz: { type: 'treemap', size: 'size', color: 'change' },
      }),
      palette,
      () => undefined,
    )
    const series = Array.isArray(option.series)
      ? option.series[0]
      : option.series

    function readOptionOutput() {
      return {
        formatter: series?.label?.formatter,
        seriesLabel: series?.label?.show,
        nodeLabels: series?.data?.map((node) => node.label?.show),
        levels: series?.levels?.length,
      }
    }
    expect(readOptionOutput()).toEqual({
      formatter: '{b}',
      seriesLabel: true,
      nodeLabels: [undefined],
      levels: 1,
    })
  })
})
