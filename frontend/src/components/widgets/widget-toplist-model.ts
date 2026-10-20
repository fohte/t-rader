import type { BarSeriesOption } from 'echarts/charts'
import type {
  AriaComponentOption,
  GridComponentOption,
  MarkLineComponentOption,
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
type WidgetToplistDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'toplist' }
>

export type WidgetToplistOption = ComposeOption<
  | AriaComponentOption
  | BarSeriesOption
  | GridComponentOption
  | MarkLineComponentOption
>

export interface WidgetToplistRow {
  key: string
  name: string
  code: string
  value: number
  formattedValue: string
}

export interface WidgetToplistScale {
  min: number
  max: number
  configured: boolean
}

export interface WidgetToplistModel {
  rows: WidgetToplistRow[]
  scale: WidgetToplistScale
  valueLabel: string
  ariaDescription: string
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function getCode(key: string): string {
  const separator = key.indexOf(':')
  return separator < 0 ? key : key.slice(separator + 1)
}

function formatToplistValue(
  name: string,
  value: number,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
): string {
  const formatted = formatWidgetValue(name, value, queries, display)
  return value > 0 && !usesWidgetPercentFormat(name, queries, display)
    ? '+' + formatted
    : formatted
}

function getValueLabel(
  name: string,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
): string {
  const label = display[name]?.label?.trim()
  if (label != null && label.length > 0) return label

  const query = queries[name]
  return query?.indicator ?? query?.metric ?? name
}

function getScale(
  rows: readonly WidgetToplistRow[],
  configured: WidgetToplistDefinition['bar_scale'],
): WidgetToplistScale {
  if (
    configured != null &&
    Number.isFinite(configured.min) &&
    Number.isFinite(configured.max) &&
    configured.min < configured.max
  ) {
    return {
      min: Math.min(0, configured.min),
      max: Math.max(0, configured.max),
      configured: true,
    }
  }

  const minimum = rows.reduce((value, row) => Math.min(value, row.value), 0)
  const maximum = rows.reduce((value, row) => Math.max(value, row.value), 0)
  if (minimum === 0 && maximum === 0) {
    return { min: -1, max: 1, configured: false }
  }

  const paddedMinimum = minimum < 0 ? minimum * 1.1 : 0
  const paddedMaximum = maximum > 0 ? maximum * 1.1 : 0
  return {
    min: Number.isFinite(paddedMinimum) ? paddedMinimum : minimum,
    max: Number.isFinite(paddedMaximum) ? paddedMaximum : maximum,
    configured: false,
  }
}

export function createWidgetToplistModel({
  result,
  viz,
  queries,
  display,
}: {
  result: WidgetQueryResponse
  viz: WidgetToplistDefinition
  queries: WidgetQueryDefinitions
  display: WidgetDisplayDefinitions
}): WidgetToplistModel {
  const rows = result.rows.flatMap((row) => {
    const value = row.values[viz.value]
    return isFiniteNumber(value)
      ? [
          {
            key: row.key,
            name: row.name,
            code: getCode(row.key),
            value,
            formattedValue: formatToplistValue(
              viz.value,
              value,
              queries,
              display,
            ),
          },
        ]
      : []
  })
  const orderedRows = rows
    .sort((left, right) => {
      const valueOrder =
        viz.order === 'asc'
          ? left.value - right.value
          : right.value - left.value
      return valueOrder || left.key.localeCompare(right.key)
    })
    .slice(0, Math.max(0, viz.limit))
  const valueLabel = getValueLabel(viz.value, queries, display)
  const orderLabel = viz.order === 'asc' ? '昇順' : '降順'

  return {
    rows: orderedRows,
    scale: getScale(orderedRows, viz.bar_scale),
    valueLabel,
    ariaDescription: `${valueLabel} の toplist。基準日 ${result.as_of}。${String(orderedRows.length)} 件を${orderLabel}で表示。${orderedRows
      .map(
        (row, index) =>
          `${String(index + 1)} 位 ${row.name}、${row.code}、${row.formattedValue}`,
      )
      .join('。')}`,
  }
}

export function createWidgetToplistOption(
  model: WidgetToplistModel,
  palette: EChartsPalette,
): WidgetToplistOption {
  const rowByKey = new Map(model.rows.map((row) => [row.key, row]))
  const rankByKey = new Map(
    model.rows.map((row, index) => [row.key, index + 1]),
  )

  return {
    aria: {
      enabled: true,
      label: { description: model.ariaDescription },
    },
    grid: {
      left: 142,
      right: 88,
      top: 8,
      bottom: 8,
      containLabel: false,
    },
    xAxis: {
      type: 'value',
      min: model.scale.min,
      max: model.scale.max,
      scale: false,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
      splitLine: {
        show: true,
        lineStyle: { color: palette.border, opacity: 0.6 },
      },
    },
    yAxis: {
      type: 'category',
      inverse: true,
      data: model.rows.map((row) => row.key),
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: palette.foreground,
        fontFamily: 'sans-serif',
        fontSize: 11,
        interval: 0,
        lineHeight: 14,
        overflow: 'truncate',
        width: 130,
        formatter: (value) => {
          const row = rowByKey.get(value)
          const rank = rankByKey.get(value)
          if (row == null || rank == null) return value
          return `${String(rank)}. ${row.name}\n${row.code}`
        },
      },
      splitLine: {
        show: true,
        lineStyle: { color: palette.border, opacity: 0.4 },
      },
    },
    series: [
      {
        id: 'widget-toplist',
        name: model.valueLabel,
        type: 'bar',
        barWidth: 12,
        clip: true,
        animation: false,
        label: {
          show: true,
          color: palette.foreground,
          fontFamily: 'monospace',
          fontSize: 11,
          distance: 4,
          formatter: (params) =>
            model.rows[params.dataIndex]?.formattedValue ?? '',
        },
        markLine: {
          silent: true,
          symbol: 'none',
          label: { show: false },
          lineStyle: { color: palette.foreground, opacity: 0.35, width: 1 },
          data: [{ xAxis: 0 }],
        },
        data: model.rows.map((row) => {
          const isBelowScale = row.value < model.scale.min
          const isAboveScale = row.value > model.scale.max
          return {
            name: row.name,
            value: Math.min(
              model.scale.max,
              Math.max(model.scale.min, row.value),
            ),
            cursor: 'pointer',
            itemStyle: {
              color:
                row.value > 0
                  ? palette.up
                  : row.value < 0
                    ? palette.down
                    : palette.neutral,
            },
            label: {
              position: isBelowScale
                ? 'insideLeft'
                : isAboveScale
                  ? 'insideRight'
                  : row.value < 0
                    ? 'left'
                    : 'right',
              ...(isBelowScale || isAboveScale
                ? { color: palette.background }
                : {}),
            },
          }
        }),
      },
    ],
  }
}
