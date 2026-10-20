import type { LabelLayoutOptionCallbackParams } from 'echarts'
import type { TreemapSeriesOption } from 'echarts/charts'
import type { AriaComponentOption } from 'echarts/components'
import type { ComposeOption } from 'echarts/core'

import type { components } from '#lib/api/schema.gen'
import type { EChartsPalette } from '#lib/echarts/use-echarts-chart'
import {
  formatWidgetValue,
  type WidgetDisplayDefinitions,
  type WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetRow = components['schemas']['WidgetQueryResultRow']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>
type WidgetGroup = NonNullable<
  NonNullable<WidgetRow['classification']>['group']
>
type WidgetScale = NonNullable<WidgetTreemapDefinition['color_scale']>

export type {
  TreemapLabelMode,
  TreemapRect,
} from '#components/widgets/widget-treemap-label'
export { getTreemapLabelMode } from '#components/widgets/widget-treemap-label'

export type WidgetTreemapOption = ComposeOption<
  TreemapSeriesOption | AriaComponentOption
>

type TreemapSeriesNode = NonNullable<TreemapSeriesOption['data']>[number]

export interface TreemapRowModel {
  key: string
  name: string
  row: WidgetRow
  groupId: string | null
  groupKey: string | null
  groupName: string | null
  sizeValue: number | null
  colorValue: number | null
  area: number
  formattedSize: string
  formattedColor: string
}

export interface TreemapGroupModel {
  id: string
  key: string | null
  name: string
  area: number
  rows: TreemapRowModel[]
}

export interface TreemapDataNode extends Omit<TreemapSeriesNode, 'children'> {
  kind: 'group' | 'row'
  groupKey?: string | null
  rowKey?: string
  children?: TreemapDataNode[]
}

export interface WidgetTreemapModel {
  rows: TreemapRowModel[]
  groups: TreemapGroupModel[]
  chartData: TreemapDataNode[]
  dataIndexToRowKey: Map<number, string>
  colorScale: WidgetScale
  sizeLabel: string
  colorLabel: string
  hierarchyAxis: string | null
}

function isFiniteNumber(value: number | null | undefined): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}

function metricLabel(
  name: string,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
): string {
  const label = display[name]?.label?.trim()
  if (label != null && label.length > 0) return label

  const query = queries[name]
  return query?.indicator ?? query?.metric ?? name
}

function formatValue(
  name: string,
  value: number | null,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions,
): string {
  return value == null ? '—' : formatWidgetValue(name, value, queries, display)
}

function getColorScale(
  values: readonly (number | null)[],
  configured: WidgetScale | null | undefined,
): WidgetScale {
  if (
    configured != null &&
    Number.isFinite(configured.min) &&
    Number.isFinite(configured.max) &&
    configured.min < configured.max
  ) {
    return configured
  }

  const maxMagnitude = values.reduce<number>(
    (maximum, value) =>
      value == null ? maximum : Math.max(maximum, Math.abs(value)),
    0,
  )
  const magnitude = maxMagnitude > 0 ? maxMagnitude : 1
  return { min: -magnitude, max: magnitude }
}

function parseHexColor(color: string): [number, number, number] | null {
  const hex = color.startsWith('#') ? color.slice(1) : color
  const expanded =
    hex.length === 3
      ? hex.replace(/./g, (character) => `${character}${character}`)
      : hex
  if (!/^[\da-f]{6}$/i.test(expanded)) return null

  return [
    Number.parseInt(expanded.slice(0, 2), 16),
    Number.parseInt(expanded.slice(2, 4), 16),
    Number.parseInt(expanded.slice(4, 6), 16),
  ]
}

function interpolateColor(start: string, end: string, amount: number): string {
  const startRgb = parseHexColor(start)
  const endRgb = parseHexColor(end)
  if (startRgb == null || endRgb == null) return start

  const channels = startRgb.map((channel, index) =>
    Math.round(channel + ((endRgb[index] ?? channel) - channel) * amount),
  )
  return `#${channels.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`
}

export function getTreemapFillColor(
  value: number | null,
  scale: WidgetScale,
  palette: EChartsPalette,
): string {
  if (value == null || value === 0) return palette.neutral

  const range = value > 0 ? scale.max : Math.abs(scale.min)
  if (range <= 0) return palette.neutral

  const intensity = Math.min(Math.abs(value) / range, 1)
  const transition = 0.35
  const dimColor = value > 0 ? palette.upDim : palette.downDim
  const strongColor = value > 0 ? palette.up : palette.down
  const target =
    intensity <= transition
      ? interpolateColor(palette.neutral, dimColor, intensity / transition)
      : interpolateColor(
          dimColor,
          strongColor,
          (intensity - transition) / (1 - transition),
        )

  return target
}

export function getTreemapTextColor(color: string): string {
  const rgb = parseHexColor(color)
  if (rgb == null) return '#fafafa'

  const luminance = rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114
  return luminance > 145 ? '#0a0a0a' : '#fafafa'
}

function getGroup(row: WidgetRow, axis: string): WidgetGroup | null {
  if (row.classification?.axis !== axis) return null
  return row.classification.group ?? null
}

function groupId(axis: string, key: string | null): string {
  return `sector:${JSON.stringify([axis, key])}`
}

function compareByArea(left: TreemapRowModel, right: TreemapRowModel): number {
  return right.area - left.area || left.key.localeCompare(right.key)
}

function createRowNode(row: TreemapRowModel): TreemapDataNode {
  return {
    id: `row:${row.key}`,
    kind: 'row',
    rowKey: row.key,
    name: row.name,
    value: row.area,
  }
}

export function createWidgetTreemapModel({
  result,
  viz,
  queries,
  display,
}: {
  result: WidgetQueryResponse
  viz: WidgetTreemapDefinition
  queries: WidgetQueryDefinitions
  display: WidgetDisplayDefinitions
}): WidgetTreemapModel {
  const colorValues = result.rows.map((row) => {
    const value = row.values[viz.color]
    return isFiniteNumber(value) ? value : null
  })
  const colorScale = getColorScale(colorValues, viz.color_scale)
  const hierarchyAxis = viz.hierarchy?.axis ?? null
  const rows = result.rows.map((row): TreemapRowModel => {
    const sizeValue = row.values[viz.size]
    const colorValue = row.values[viz.color]
    const normalizedSize = isFiniteNumber(sizeValue) ? sizeValue : null
    const normalizedColor = isFiniteNumber(colorValue) ? colorValue : null
    const category = hierarchyAxis == null ? null : getGroup(row, hierarchyAxis)

    return {
      key: row.key,
      name: row.name,
      row,
      groupId:
        hierarchyAxis == null
          ? null
          : groupId(hierarchyAxis, category?.key ?? null),
      groupKey: category?.key ?? null,
      groupName: category?.name ?? (hierarchyAxis == null ? null : '未分類'),
      sizeValue: normalizedSize,
      colorValue: normalizedColor,
      area: normalizedSize == null ? 0 : Math.abs(normalizedSize),
      formattedSize: formatValue(viz.size, normalizedSize, queries, display),
      formattedColor: formatValue(viz.color, normalizedColor, queries, display),
    }
  })

  const chartRows = rows.filter((row) => row.area > 0).sort(compareByArea)
  const groups = new Map<string, TreemapGroupModel>()
  if (hierarchyAxis != null) {
    for (const row of rows) {
      const id = row.groupId
      if (id == null) continue
      const existing = groups.get(id)
      if (existing != null) {
        existing.rows.push(row)
        existing.area += row.area
        continue
      }

      groups.set(id, {
        id,
        key: row.groupKey,
        name: row.groupName ?? '未分類',
        area: row.area,
        rows: [row],
      })
    }
  }

  const sortedGroups = [...groups.values()]
    .map((group) => ({
      ...group,
      rows: [...group.rows].sort(compareByArea),
    }))
    .sort(
      (left, right) =>
        right.area - left.area || left.id.localeCompare(right.id),
    )

  const chartData =
    hierarchyAxis == null
      ? chartRows.map((row) => createRowNode(row))
      : sortedGroups.flatMap((group): TreemapDataNode[] => {
          const children = group.rows
            .filter((row) => row.area > 0)
            .map((row) => createRowNode(row))
          if (children.length === 0) return []
          return [
            {
              id: group.id,
              kind: 'group',
              groupKey: group.key,
              name: group.name,
              value: group.area,
              children,
            },
          ]
        })

  const dataIndexToRowKey = new Map<number, string>()
  let dataIndex = 1
  const indexNodes = (nodes: readonly TreemapDataNode[]) => {
    for (const node of nodes) {
      const currentIndex = dataIndex
      dataIndex += 1
      if (node.kind === 'row' && node.rowKey != null) {
        dataIndexToRowKey.set(currentIndex, node.rowKey)
      }
      if (node.children != null) indexNodes(node.children)
    }
  }
  indexNodes(chartData)

  return {
    rows: [...rows].sort(compareByArea),
    groups: sortedGroups,
    chartData,
    dataIndexToRowKey,
    colorScale,
    sizeLabel: metricLabel(viz.size, queries, display),
    colorLabel: metricLabel(viz.color, queries, display),
    hierarchyAxis,
  }
}

export function createWidgetTreemapOption(
  model: WidgetTreemapModel,
  palette: EChartsPalette,
  onNodeLayout: (
    rowKey: string,
    rect: LabelLayoutOptionCallbackParams['rect'],
  ) => void,
): WidgetTreemapOption {
  const groupForeground = getTreemapTextColor(palette.neutral)
  const rowByKey = new Map(model.rows.map((row) => [row.key, row]))
  const styleRowNode = (node: TreemapDataNode): TreemapDataNode => {
    const row = node.rowKey == null ? undefined : rowByKey.get(node.rowKey)
    return {
      ...node,
      cursor: 'pointer',
      itemStyle: {
        color:
          row == null
            ? palette.neutral
            : getTreemapFillColor(row.colorValue, model.colorScale, palette),
      },
    }
  }
  const chartData = model.chartData.map((node): TreemapDataNode =>
    node.kind === 'group'
      ? {
          ...node,
          cursor: 'pointer',
          children: node.children?.map(styleRowNode),
        }
      : styleRowNode(node),
  )
  const groupItemStyle = {
    color: palette.neutral,
    borderColor: palette.border,
  }

  return {
    aria: {
      enabled: true,
      label: {
        description: `${model.sizeLabel} の値で面積を表し、${model.colorLabel} の値を赤から青の濃淡で表します。`,
      },
    },
    series: [
      {
        id: 'widget-treemap',
        name: 'widget-treemap',
        type: 'treemap',
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
        sort: false,
        roam: false,
        nodeClick: false,
        breadcrumb: { show: false },
        animation: false,
        label: {
          show: true,
          formatter: '{b}',
          color: 'rgba(0, 0, 0, 0)',
          fontSize: 1,
          padding: 0,
        },
        labelLayout: (params) => {
          if (params.dataIndex == null) return {}
          const rowKey = model.dataIndexToRowKey.get(params.dataIndex)
          if (rowKey != null) {
            onNodeLayout(rowKey, {
              x: params.rect.x,
              y: params.rect.y,
              width: params.rect.width,
              height: params.rect.height,
            })
          }
          return {}
        },
        upperLabel: {
          show: false,
        },
        levels:
          model.hierarchyAxis == null
            ? [
                {
                  itemStyle: {
                    borderColor: palette.background,
                    borderWidth: 1,
                    gapWidth: 2,
                  },
                },
              ]
            : [
                {
                  itemStyle: {
                    ...groupItemStyle,
                    borderWidth: 0,
                    gapWidth: 0,
                  },
                },
                {
                  label: { show: false },
                  itemStyle: {
                    ...groupItemStyle,
                    borderWidth: 1,
                    gapWidth: 4,
                  },
                  upperLabel: {
                    show: true,
                    formatter: '{b}',
                    height: 22,
                    color: groupForeground,
                    backgroundColor: palette.neutral,
                    fontSize: 11,
                    fontWeight: 'bold',
                    padding: [3, 6],
                    overflow: 'truncate',
                  },
                },
                {
                  itemStyle: {
                    borderColor: palette.background,
                    borderWidth: 1,
                    gapWidth: 2,
                  },
                },
              ],
        data: chartData,
      },
    ],
  }
}
