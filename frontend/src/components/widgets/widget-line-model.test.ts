import { describe, expect, it } from 'vitest'

import {
  createWidgetLineModel,
  createWidgetLineOption,
} from '#components/widgets/widget-line-model'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { testEChartsPalette } from '#test/echarts-palette.test-helper'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetLineDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'line' }
>

const queries: WidgetQueryDefinitions = {
  movement: { metric: 'change', period: '20d', by: 'group' },
  indicator: {
    metric: 'indicator',
    by: 'none',
    indicator: 'demo-indicator',
  },
}
const display = {
  movement: { label: '騰落率' },
  indicator: { label: '水準値', format: 'number' },
} satisfies components['schemas']['WidgetQueryRequest']['display']
const viz: WidgetLineDefinition = {
  type: 'line',
  series: ['movement', 'indicator'],
}
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
        { date: '2099-01-07', value: 0.02 },
        { date: '2099-01-05', value: 0.01 },
        { date: '2099-01-06', value: null },
      ],
    },
    {
      key: 'indicator',
      query: 'indicator',
      name: '水準値',
      points: [
        { date: '2099-01-19', value: 0.5 },
        { date: '2099-01-05', value: 0 },
      ],
    },
  ],
}

function createModel() {
  return createWidgetLineModel({ result, viz, queries, display })
}

function readOptionOutput() {
  const model = createModel()
  const option = createWidgetLineOption(
    model,
    queries,
    display,
    testEChartsPalette,
  )
  const xAxis = Array.isArray(option.xAxis) ? option.xAxis[0] : option.xAxis
  const yAxes = Array.isArray(option.yAxis) ? option.yAxis : [option.yAxis]
  const legend = Array.isArray(option.legend) ? option.legend[0] : option.legend
  const series =
    option.series == null
      ? []
      : Array.isArray(option.series)
        ? option.series
        : [option.series]

  return {
    color: option.color,
    grid: option.grid,
    legend: {
      type: legend?.type,
      bottom: legend?.bottom,
    },
    xAxis: {
      type: xAxis?.type,
      dateLabel:
        typeof xAxis?.axisLabel?.formatter === 'function'
          ? Reflect.apply(xAxis.axisLabel.formatter, undefined, [
              Date.parse('2099-01-05T00:00:00Z'),
              0,
              null,
            ])
          : null,
    },
    yAxis: yAxes.map((axis) => ({
      name: axis?.name,
      position: axis?.position,
      label:
        typeof axis?.axisLabel?.formatter === 'function'
          ? 'formatter'
          : (axis?.axisLabel?.formatter ?? null),
      sampleLabel:
        typeof axis?.axisLabel?.formatter === 'function'
          ? Reflect.apply(axis.axisLabel.formatter, undefined, [
              axis.name === '%' ? 0.01 : 1.5,
              0,
              null,
            ])
          : null,
    })),
    series: series.map((series) => ({
      id: series.id,
      name: series.name,
      type: series.type,
      yAxisIndex: series.yAxisIndex,
      connectNulls: series.connectNulls,
      showSymbol: series.showSymbol,
      symbolSize: series.symbolSize,
      color: series.lineStyle?.color,
      data: series.data,
      formattedValues:
        typeof series.tooltip?.valueFormatter === 'function'
          ? [
              series.tooltip.valueFormatter(0.01, 0),
              series.tooltip.valueFormatter(null, 1),
            ]
          : null,
    })),
  }
}

describe('createWidgetLineModel', () => {
  it('keeps observation dates, null gaps, zero values, labels, and units', () => {
    expect(createModel()).toEqual({
      series: [
        {
          key: 'movement:group:demo-axis/demo-group-a',
          query: 'movement',
          name: '騰落率 · サンプル分類 A',
          format: 'percent',
          points: [
            ['2099-01-05', 0.01],
            ['2099-01-06', null],
            ['2099-01-07', 0.02],
          ],
        },
        {
          key: 'indicator',
          query: 'indicator',
          name: '水準値',
          format: 'number',
          points: [
            ['2099-01-05', 0],
            ['2099-01-19', 0.5],
          ],
        },
      ],
      formats: ['percent', 'number'],
      startDate: '2099-01-05',
      endDate: '2099-01-19',
      hasValues: true,
      ariaDescription:
        '2099-01-05 から 2099-01-19の折れ線グラフ。系列: 騰落率 · サンプル分類 A、水準値。欠損値は線をつなぎません。',
    })
  })
})

describe('createWidgetLineOption', () => {
  it('shows date ticks, separate units, formatted values, and disconnected null points', () => {
    expect(readOptionOutput()).toEqual({
      color: testEChartsPalette.seriesColors,
      grid: {
        left: 62,
        right: 62,
        top: 20,
        bottom: 64,
        containLabel: true,
      },
      legend: { type: 'scroll', bottom: 0 },
      xAxis: { type: 'time', dateLabel: '2099/01/05' },
      yAxis: [
        {
          name: '%',
          position: 'left',
          label: 'formatter',
          sampleLabel: '+1.00%',
        },
        {
          name: undefined,
          position: 'right',
          label: 'formatter',
          sampleLabel: '1.5',
        },
      ],
      series: [
        {
          id: 'movement:group:demo-axis/demo-group-a',
          name: '騰落率 · サンプル分類 A',
          type: 'line',
          yAxisIndex: 0,
          connectNulls: false,
          showSymbol: true,
          symbolSize: 4,
          color: testEChartsPalette.seriesColors[0],
          data: [
            [Date.parse('2099-01-05T00:00:00Z'), 0.01],
            [Date.parse('2099-01-06T00:00:00Z'), null],
            [Date.parse('2099-01-07T00:00:00Z'), 0.02],
          ],
          formattedValues: ['+1.00%', '—'],
        },
        {
          id: 'indicator',
          name: '水準値',
          type: 'line',
          yAxisIndex: 1,
          connectNulls: false,
          showSymbol: true,
          symbolSize: 4,
          color: testEChartsPalette.seriesColors[1],
          data: [
            [Date.parse('2099-01-05T00:00:00Z'), 0],
            [Date.parse('2099-01-19T00:00:00Z'), 0.5],
          ],
          formattedValues: ['0.01', '—'],
        },
      ],
    })
  })

  it('hides markers on dense series and shows them on sparse series', () => {
    const densePoints = Array.from({ length: 31 }, (_, index) => ({
      date: `2099-01-${String(index + 1).padStart(2, '0')}`,
      value: index / 100,
    }))
    const denseModel = createWidgetLineModel({
      result: {
        as_of: '2099-01-31',
        rows: [],
        series: [
          {
            key: 'dense',
            query: 'indicator',
            name: '水準値',
            points: densePoints,
          },
        ],
      },
      viz,
      queries,
      display,
    })
    const option = createWidgetLineOption(
      denseModel,
      queries,
      display,
      testEChartsPalette,
    )
    const optionSeries = Array.isArray(option.series)
      ? option.series
      : option.series == null
        ? []
        : [option.series]

    expect(
      optionSeries.map(({ name, showSymbol, symbolSize }) => ({
        name,
        showSymbol,
        symbolSize,
      })),
    ).toEqual([{ name: '水準値', showSymbol: false, symbolSize: 4 }])
  })
})
