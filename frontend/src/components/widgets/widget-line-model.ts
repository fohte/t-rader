import type { LineSeriesOption } from 'echarts/charts'
import type {
  AriaComponentOption,
  GridComponentOption,
  LegendComponentOption,
  TooltipComponentOption,
} from 'echarts/components'
import type { ComposeOption } from 'echarts/core'

import type { components } from '#lib/api/schema.gen'
import type { EChartsPalette } from '#lib/echarts/use-echarts-chart'
import {
  formatWidgetValue,
  usesWidgetPercentFormat,
  type WidgetDisplayDefinitions,
  type WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetLineDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'line' }
>

export type WidgetLineOption = ComposeOption<
  | AriaComponentOption
  | GridComponentOption
  | LegendComponentOption
  | LineSeriesOption
  | TooltipComponentOption
>

type WidgetLineFormat = 'number' | 'percent'
const MAX_VISIBLE_SYMBOLS = 30

interface WidgetLineSeriesModel {
  key: string
  query: string
  name: string
  format: WidgetLineFormat
  points: [string, number | null][]
}

export interface WidgetLineModel {
  series: WidgetLineSeriesModel[]
  formats: WidgetLineFormat[]
  startDate: string | null
  endDate: string | null
  hasValues: boolean
  ariaDescription: string
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function createSeriesName(
  name: string,
  targetName: string | undefined,
): string {
  return targetName == null ? name : `${name} · ${targetName}`
}

export function createWidgetLineModel({
  result,
  viz,
  queries,
  display,
}: {
  result: WidgetQueryResponse
  viz: WidgetLineDefinition
  queries: WidgetQueryDefinitions
  display: WidgetDisplayDefinitions
}): WidgetLineModel {
  const series = (result.series ?? []).map((entry) => {
    const displayLabel = display[entry.query]?.label?.trim()
    const label =
      displayLabel == null || displayLabel === '' ? entry.name : displayLabel
    const format: WidgetLineFormat = usesWidgetPercentFormat(
      entry.query,
      queries,
      display,
    )
      ? 'percent'
      : 'number'
    return {
      key: entry.key,
      query: entry.query,
      name: createSeriesName(label, entry.target?.name),
      format,
      points: entry.points
        .map(({ date, value }): [string, number | null] => [
          date,
          isFiniteNumber(value) ? value : null,
        ])
        .sort(([left], [right]) => left.localeCompare(right)),
    }
  })
  const formats = [
    ...new Set<WidgetLineFormat>(series.map(({ format }) => format)),
  ]
  const dates = series.flatMap(({ points }) => points.map(([date]) => date))
  const startDate =
    dates.length === 0 ? null : dates.reduce((a, b) => (a < b ? a : b))
  const endDate =
    dates.length === 0 ? null : dates.reduce((a, b) => (a > b ? a : b))
  const hasValues = series.some(({ points }) =>
    points.some(([, value]) => value != null),
  )
  const seriesNames = series.map(({ name }) => name)
  const dateRange =
    startDate == null || endDate == null
      ? '日付データなし'
      : `${startDate} から ${endDate}`

  return {
    series,
    formats,
    startDate,
    endDate,
    hasValues,
    ariaDescription: `${dateRange}の折れ線グラフ。系列: ${seriesNames.join('、') || viz.series.join('、')}。欠損値は線をつなぎません。`,
  }
}

function toTimestamp(date: string): number | string {
  const timestamp = Date.parse(`${date}T00:00:00Z`)
  return Number.isFinite(timestamp) ? timestamp : date
}

function formatDateTick(value: string | number): string {
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return String(value)

  return `${String(date.getUTCFullYear()).padStart(4, '0')}/${String(date.getUTCMonth() + 1).padStart(2, '0')}/${String(date.getUTCDate()).padStart(2, '0')}`
}

function createValueFormatter(
  name: string,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
) {
  return (value: unknown) => {
    const candidate = Array.isArray(value) ? value[1] : value
    return isFiniteNumber(candidate)
      ? formatWidgetValue(name, candidate, queries, display)
      : '—'
  }
}

export function createWidgetLineOption(
  model: WidgetLineModel,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
  palette: EChartsPalette,
): WidgetLineOption {
  const hasMixedFormats = model.formats.length > 1
  const yAxis = model.formats.map((format, index) => {
    const query = model.series.find((series) => series.format === format)?.query
    return {
      type: 'value' as const,
      position:
        hasMixedFormats && index > 0 ? ('right' as const) : ('left' as const),
      scale: true,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: palette.foreground,
        formatter: (value: string | number) =>
          query != null && typeof value === 'number'
            ? formatWidgetValue(query, value, queries, display)
            : String(value),
      },
      splitLine: {
        show: index === 0,
        lineStyle: { color: palette.border, opacity: 0.6 },
      },
      name: format === 'percent' ? '%' : undefined,
      nameTextStyle: { color: palette.foreground },
    }
  })

  return {
    aria: {
      enabled: true,
      label: { description: model.ariaDescription },
    },
    color: palette.seriesColors,
    grid: {
      left: hasMixedFormats ? 62 : 52,
      right: hasMixedFormats ? 62 : 20,
      top: 20,
      bottom: 64,
      containLabel: true,
    },
    legend: {
      type: 'scroll',
      bottom: 0,
      left: 0,
      right: 0,
      textStyle: { color: palette.foreground },
      pageIconColor: palette.foreground,
      pageTextStyle: { color: palette.foreground },
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' },
      backgroundColor: palette.background,
      borderColor: palette.border,
      textStyle: { color: palette.foreground },
    },
    xAxis: {
      type: 'time',
      axisLine: { lineStyle: { color: palette.border } },
      axisTick: { show: false },
      axisLabel: {
        color: palette.foreground,
        formatter: formatDateTick,
        hideOverlap: true,
      },
      splitLine: {
        show: false,
      },
    },
    yAxis,
    series: model.series.map((series, index) => ({
      id: series.key,
      name: series.name,
      type: 'line' as const,
      yAxisIndex: model.formats.indexOf(series.format),
      animation: false,
      clip: true,
      connectNulls: false,
      showSymbol:
        series.points.filter(([, value]) => value != null).length <=
        MAX_VISIBLE_SYMBOLS,
      symbolSize: 4,
      emphasis: { focus: 'series' as const },
      lineStyle: {
        color: palette.seriesColors[index % palette.seriesColors.length],
        width: 2,
      },
      itemStyle: {
        color: palette.seriesColors[index % palette.seriesColors.length],
      },
      tooltip: {
        valueFormatter: createValueFormatter(series.query, queries, display),
      },
      data: series.points.map(([date, value]) => [toTimestamp(date), value]),
    })),
  }
}
