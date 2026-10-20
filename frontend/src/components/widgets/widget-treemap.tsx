import '#lib/echarts/treemap'

import { Link } from '@tanstack/react-router'
import type { ECElementEvent } from 'echarts/core'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import {
  createWidgetTreemapModel,
  createWidgetTreemapOption,
  getTreemapFillColor,
  getTreemapTextColor,
  type TreemapDataNode,
  type TreemapRect,
  type TreemapRowModel,
} from '#components/widgets/widget-treemap-model'
import type { components } from '#lib/api/schema.gen'
import { useEChartsChart } from '#lib/echarts/use-echarts-chart'
import { parseGroupRefId } from '#lib/ref-id'
import {
  formatWidgetValue,
  type WidgetDisplayDefinitions,
  type WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const EMPTY_DISPLAY: WidgetDisplayDefinitions = {}
const SERIES_ID = 'widget-treemap'
const FALLBACK_PALETTE = {
  background: '#fafafa',
  border: '#e5e5e5',
  foreground: '#0a0a0a',
  neutral: '#1a1a1a',
  up: '#ef5350',
  upDim: '#5a2a2a',
  down: '#3f9fe0',
  downDim: '#234152',
}

function isTreemapDataNode(value: unknown): value is TreemapDataNode {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    (value.kind === 'group' || value.kind === 'row')
  )
}

function isSameLayout(
  left: ReadonlyMap<string, TreemapRect>,
  right: ReadonlyMap<string, TreemapRect>,
): boolean {
  if (left.size !== right.size) return false
  for (const [key, rect] of right) {
    const previous = left.get(key)
    if (
      previous == null ||
      previous.x !== rect.x ||
      previous.y !== rect.y ||
      previous.width !== rect.width ||
      previous.height !== rect.height
    ) {
      return false
    }
  }
  return true
}

function getLabelMode(
  rect: TreemapRect,
  value: string,
): 'both' | 'value' | null {
  const availableWidth = rect.width - 8
  const availableHeight = rect.height - 8
  const valueWidth = value.length * 7
  if (availableWidth < valueWidth) return null
  if (availableHeight >= 32) return 'both'
  if (availableHeight >= 18) return 'value'
  return null
}

export function WidgetTreemap({
  result,
  viz,
  queries,
  display,
  initialFocusGroupKey,
  initialSelectedRowKey,
}: {
  result: WidgetQueryResponse
  viz: WidgetTreemapDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
  initialFocusGroupKey?: string
  initialSelectedRowKey?: string
}) {
  const displayDefinitions = display ?? EMPTY_DISPLAY
  const labelRectsRef = useRef(new Map<string, TreemapRect>())
  const [labelRects, setLabelRects] = useState<
    ReadonlyMap<string, TreemapRect>
  >(new Map())

  const [selectedRowKey, setSelectedRowKey] = useState<string | null>(
    initialSelectedRowKey ?? null,
  )
  const model = useMemo(
    () =>
      createWidgetTreemapModel({
        result,
        viz,
        queries,
        display: displayDefinitions,
      }),
    [displayDefinitions, queries, result, viz],
  )

  const getInitialFocusId = () =>
    initialFocusGroupKey == null
      ? null
      : (model.groups.find((group) => group.key === initialFocusGroupKey)?.id ??
        null)
  const [focusId, setFocusId] = useState<string | null>(getInitialFocusId)
  const focusedGroup =
    focusId == null
      ? null
      : (model.groups.find((group) => group.id === focusId) ?? null)

  const clearLabelRects = useCallback(() => {
    labelRectsRef.current.clear()
  }, [])
  const recordLabelRect = useCallback((rowKey: string, rect: TreemapRect) => {
    labelRectsRef.current.set(rowKey, rect)
  }, [])
  const handleChartRendered = useCallback(() => {
    const visibleKeys = new Set(
      model.rows.filter((row) => row.area > 0).map((row) => row.key),
    )
    const nextRects = new Map(
      [...labelRectsRef.current].filter(([key]) => visibleKeys.has(key)),
    )
    setLabelRects((previous) =>
      isSameLayout(previous, nextRects) ? previous : nextRects,
    )
  }, [model.rows])

  const fallbackOption = useMemo(
    () => createWidgetTreemapOption(model, FALLBACK_PALETTE, recordLabelRect),
    [model, recordLabelRect],
  )
  const { containerRef, palette, error, updateOption, dispatchAction } =
    useEChartsChart({
      option: fallbackOption,
      onClick: (event: ECElementEvent) => {
        const node = event.data
        if (!isTreemapDataNode(node)) return
        if (node.kind === 'group') {
          setSelectedRowKey(null)
          setFocusId(node.id == null ? null : String(node.id))
          return
        }
        if (node.rowKey != null) setSelectedRowKey(node.rowKey)
      },
      onRendered: handleChartRendered,
      onBeforeUpdate: clearLabelRects,
    })

  const option = useMemo(
    () => createWidgetTreemapOption(model, palette, recordLabelRect),
    [model, palette, recordLabelRect],
  )

  useEffect(() => {
    updateOption(option)
  }, [option, updateOption])

  useEffect(() => {
    if (focusId == null) return
    if (focusedGroup == null) {
      setFocusId(null)
      return
    }
    clearLabelRects()
    dispatchAction({
      type: 'treemapRootToNode',
      seriesId: SERIES_ID,
      targetNodeId: focusedGroup.id,
    })
  }, [clearLabelRects, dispatchAction, focusId, focusedGroup, option])

  const rowsByKey = useMemo(
    () => new Map(model.rows.map((row) => [row.key, row])),
    [model.rows],
  )

  const selectedRow =
    selectedRowKey == null
      ? null
      : (model.rows.find((row) => row.key === selectedRowKey) ?? null)
  const stockId =
    selectedRow?.key.startsWith('stock:') === true
      ? selectedRow.key.slice('stock:'.length)
      : null
  const groupTarget =
    selectedRow?.key.startsWith('group:') === true
      ? parseGroupRefId(selectedRow.key.slice('group:'.length))
      : null
  const colorMin = formatWidgetValue(
    viz.color,
    model.colorScale.min,
    queries,
    displayDefinitions,
  )
  const colorMax = formatWidgetValue(
    viz.color,
    model.colorScale.max,
    queries,
    displayDefinitions,
  )

  const selectRow = (row: TreemapRowModel) => {
    setSelectedRowKey(row.key)
  }
  const focusGroup = (groupId: string) => {
    setSelectedRowKey(null)
    setFocusId(groupId)
  }
  const resetToOverview = () => {
    clearLabelRects()
    updateOption(option)
    setFocusId(null)
  }

  const chartRows = model.rows.filter((row) => row.area > 0)
  const renderRowButtons = (rows: readonly TreemapRowModel[]) =>
    rows.map((row) => (
      <li className="border-t border-border first:border-t-0" key={row.key}>
        <button
          aria-label={`${row.name}, ${model.colorLabel}: ${row.formattedColor}, 詳細を表示`}
          className="flex min-h-10 w-full items-center gap-3 px-3 py-2 text-left hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
          onClick={() => {
            selectRow(row)
          }}
          type="button"
        >
          <span className="min-w-0 flex-1 break-words text-foreground">
            {row.name}
          </span>
          <span className="shrink-0 font-mono tabular-nums text-muted-foreground">
            {row.formattedColor}
          </span>
        </button>
      </li>
    ))

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span>面積: {model.sizeLabel}</span>
          <span className="inline-flex items-center gap-2">
            <span>色: {model.colorLabel}</span>
            <span
              aria-hidden="true"
              className="h-2 w-28 border border-border"
              style={{
                background: `linear-gradient(90deg, ${palette.down}, ${palette.neutral} 50%, ${palette.up})`,
              }}
            />
            <span className="font-mono tabular-nums">
              {colorMin} / {colorMax}
            </span>
          </span>
        </div>
        <button
          className="min-h-9 border border-border px-3 text-xs text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
          disabled={focusId == null}
          onClick={resetToOverview}
          type="button"
        >
          全体を表示
        </button>
      </div>

      <nav
        aria-label="表示中の階層"
        className="flex min-h-8 items-center gap-2 text-xs"
      >
        <button
          aria-current={focusedGroup == null ? 'page' : undefined}
          className="text-foreground underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-ring"
          disabled={focusedGroup == null}
          onClick={resetToOverview}
          type="button"
        >
          すべての分類
        </button>
        {focusedGroup != null && (
          <>
            <span aria-hidden="true" className="text-muted-foreground">
              /
            </span>
            <span aria-current="page" className="text-foreground">
              {focusedGroup.name}
            </span>
          </>
        )}
      </nav>

      <div className="relative aspect-video min-h-52 w-full overflow-hidden border border-border bg-card">
        <div
          aria-hidden={chartRows.length === 0}
          aria-label={`${model.sizeLabel} の値で面積を表す treemap。色は ${model.colorLabel} の値です。`}
          className="absolute inset-0"
          ref={containerRef}
          role="img"
        />
        {chartRows.length === 0 ? (
          <p
            className="absolute inset-0 flex items-center justify-center px-4 text-center font-mono text-xs text-muted-foreground"
            role="status"
          >
            表示できるデータがありません。
          </p>
        ) : (
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-0"
          >
            {[...labelRects].flatMap(([key, rect]) => {
              const row = rowsByKey.get(key)
              if (row == null) return []
              const mode = getLabelMode(rect, row.formattedColor)
              if (mode == null) return []

              return [
                <div
                  className="absolute flex flex-col justify-center overflow-hidden px-1 text-center"
                  key={key}
                  style={{
                    left: rect.x,
                    top: rect.y,
                    width: rect.width,
                    height: rect.height,
                    color: getTreemapTextColor(
                      getTreemapFillColor(
                        row.colorValue,
                        model.colorScale,
                        palette,
                      ),
                    ),
                  }}
                >
                  {mode === 'both' && (
                    <span className="min-w-0 truncate font-sans text-xs font-semibold leading-tight">
                      {row.name}
                    </span>
                  )}
                  <span className="whitespace-nowrap font-mono text-xs font-semibold leading-tight tabular-nums">
                    {row.formattedColor}
                  </span>
                </div>,
              ]
            })}
          </div>
        )}
      </div>
      {error != null && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}

      {selectedRow != null && (
        <section
          aria-label="選択した行の詳細"
          className="border border-border bg-card p-3"
        >
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0">
              <h3 className="break-words text-sm font-semibold text-foreground">
                {selectedRow.name}
              </h3>
              <dl
                className="mt-2 grid gap-x-4 gap-y-1 text-xs"
                style={{ gridTemplateColumns: 'auto 1fr' }}
              >
                <dt className="text-muted-foreground">
                  面積: {model.sizeLabel}
                </dt>
                <dd className="text-right font-mono tabular-nums text-foreground">
                  {selectedRow.formattedSize}
                </dd>
                <dt className="text-muted-foreground">
                  色: {model.colorLabel}
                </dt>
                <dd className="text-right font-mono tabular-nums text-foreground">
                  {selectedRow.formattedColor}
                </dd>
              </dl>
            </div>
            <div className="flex items-center gap-3">
              {stockId != null && (
                <Link
                  className="min-h-9 border border-border px-3 py-2 text-xs text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                  params={{ stockId }}
                  to="/stocks/$stockId"
                >
                  銘柄ページを開く
                </Link>
              )}
              {groupTarget != null && (
                <Link
                  className="min-h-9 border border-border px-3 py-2 text-xs text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                  params={groupTarget}
                  to="/groups/$axisKey/$groupKey"
                >
                  分類ページを開く
                </Link>
              )}
              <button
                aria-label="詳細を閉じる"
                className="min-h-9 px-2 text-xs text-muted-foreground hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring"
                onClick={() => {
                  setSelectedRowKey(null)
                }}
                type="button"
              >
                閉じる
              </button>
            </div>
          </div>
        </section>
      )}

      {model.rows.length > 0 && (
        <details className="border border-border bg-card">
          <summary className="min-h-10 cursor-pointer px-3 py-2 text-xs text-foreground focus-visible:outline-2 focus-visible:outline-ring">
            全件一覧 ({model.rows.length})
          </summary>
          <div className="border-t border-border">
            {model.groups.length > 0 ? (
              <div className="divide-y divide-border">
                {model.groups.map((group) => (
                  <section key={group.id}>
                    <div className="flex min-h-10 items-start justify-between gap-3 bg-bg-secondary px-3 py-2">
                      <h3 className="min-w-0 flex-1 break-words text-xs font-semibold text-foreground">
                        {group.name}
                      </h3>
                      <button
                        aria-pressed={focusId === group.id}
                        className="shrink-0 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline focus-visible:outline-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50"
                        disabled={group.area <= 0}
                        onClick={() => {
                          focusGroup(group.id)
                        }}
                        type="button"
                      >
                        分類にフォーカス
                      </button>
                    </div>
                    <ul
                      aria-label={`${group.name} の銘柄`}
                      className="list-none"
                    >
                      {renderRowButtons(group.rows)}
                    </ul>
                  </section>
                ))}
              </div>
            ) : (
              <ul aria-label="treemap の全件" className="list-none">
                {renderRowButtons(model.rows)}
              </ul>
            )}
          </div>
        </details>
      )}
    </div>
  )
}
