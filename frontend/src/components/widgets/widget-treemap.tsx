import '#lib/echarts/treemap'

import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'
import type { ECElementEvent } from 'echarts/core'
import { ChevronDown } from 'lucide-react'
import {
  type CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'

import {
  createWidgetTreemapModel,
  createWidgetTreemapOption,
  getTreemapFillColor,
  getTreemapLabelMode,
  getTreemapTextColor,
  type TreemapRect,
  type TreemapRowModel,
} from '#components/widgets/widget-treemap-model'
import {
  isSameLayout,
  isTreemapDataNode,
} from '#components/widgets/widget-treemap-utils'
import type { components } from '#lib/api/schema.gen'
import {
  type EChartsPalette,
  useEChartsChart,
} from '#lib/echarts/use-echarts-chart'
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
type WidgetCSSProperties = CSSProperties & Record<`--widget-${string}`, string>

const EMPTY_DISPLAY: WidgetDisplayDefinitions = {}
const SERIES_ID = 'widget-treemap'
const DETAIL_STYLE: WidgetCSSProperties = {
  '--widget-detail-columns': 'auto 1fr',
}

export function WidgetTreemap({
  result,
  viz,
  queries,
  display,
  initialFocusGroupKey,
  initialSelectedRowKey,
  initialListExpanded,
}: {
  result: WidgetQueryResponse
  viz: WidgetTreemapDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
  initialFocusGroupKey?: string
  initialSelectedRowKey?: string
  initialListExpanded?: boolean
}) {
  const displayDefinitions = display ?? EMPTY_DISPLAY
  const labelRectsRef = useRef(new Map<string, TreemapRect>())
  const [labelRects, setLabelRects] = useState<
    ReadonlyMap<string, TreemapRect>
  >(new Map())

  const [selectedRowKey, setSelectedRowKey] = useState<string | null>(
    initialSelectedRowKey ?? null,
  )
  const [listExpanded, setListExpanded] = useState(initialListExpanded ?? false)
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
    setLabelRects((previous) => (previous.size === 0 ? previous : new Map()))
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

  const option = useMemo(
    () => (activePalette: EChartsPalette) =>
      createWidgetTreemapOption(model, activePalette, recordLabelRect),
    [model, recordLabelRect],
  )
  const { containerRef, palette, error, updateOption, dispatchAction } =
    useEChartsChart({
      createOption: option,
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
  }, [clearLabelRects, dispatchAction, focusId, focusedGroup, option, palette])

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
    updateOption()
    setFocusId(null)
  }

  const chartRows = model.rows.filter((row) => row.area > 0)
  const gradientStyle: WidgetCSSProperties = {
    '--widget-treemap-gradient': `linear-gradient(90deg, ${palette.down}, ${palette.neutral} 50%, ${palette.up})`,
  }
  const renderRowButtons = (rows: readonly TreemapRowModel[]) =>
    rows.map((row) => (
      <li className="border-t border-border first:border-t-0" key={row.key}>
        <Button
          aria-label={`${row.name}, ${model.colorLabel}: ${row.formattedColor}, 詳細を表示`}
          className="min-h-10 w-full justify-start text-left"
          onClick={() => {
            selectRow(row)
          }}
          size="lg"
          type="button"
          variant="ghost"
        >
          <span className="min-w-0 flex-1 break-words text-foreground">
            {row.name}
          </span>
          <span className="shrink-0 font-mono tabular-nums text-muted-foreground">
            {row.formattedColor}
          </span>
        </Button>
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
              className="h-2 w-28 border border-border bg-(image:--widget-treemap-gradient)"
              style={gradientStyle}
            />
            <span className="font-mono tabular-nums">
              {colorMin} / {colorMax}
            </span>
          </span>
        </div>
        <Button
          disabled={focusId == null}
          onClick={resetToOverview}
          type="button"
          variant="outline"
        >
          全体を表示
        </Button>
      </div>

      <nav
        aria-label="表示中の階層"
        className="flex min-h-8 items-center gap-2 text-xs"
      >
        <Button
          aria-current={focusedGroup == null ? 'page' : undefined}
          disabled={focusedGroup == null}
          onClick={resetToOverview}
          type="button"
          variant="ghost"
        >
          すべての分類
        </Button>
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
              const mode = getTreemapLabelMode(
                rect,
                row.name,
                row.formattedColor,
              )
              if (mode == null) return []

              const labelStyle: WidgetCSSProperties = {
                '--widget-treemap-label-left': String(rect.x) + 'px',
                '--widget-treemap-label-top': String(rect.y) + 'px',
                '--widget-treemap-label-width': String(rect.width) + 'px',
                '--widget-treemap-label-height': String(rect.height) + 'px',
                '--widget-treemap-label-color': getTreemapTextColor(
                  getTreemapFillColor(
                    row.colorValue,
                    model.colorScale,
                    palette,
                  ),
                  palette,
                ),
              }

              return [
                <div
                  className="absolute flex h-(--widget-treemap-label-height) w-(--widget-treemap-label-width) flex-col justify-center overflow-hidden px-1 text-center text-(color:--widget-treemap-label-color) left-(--widget-treemap-label-left) top-(--widget-treemap-label-top)"
                  key={key}
                  style={labelStyle}
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
                className="mt-2 grid grid-cols-(--widget-detail-columns) gap-x-4 gap-y-1 text-xs"
                style={DETAIL_STYLE}
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
                <Button
                  nativeButton={false}
                  role="link"
                  render={<Link params={{ stockId }} to="/stocks/$stockId" />}
                  size="lg"
                  variant="outline"
                >
                  銘柄ページを開く
                </Button>
              )}
              {groupTarget != null && (
                <Button
                  nativeButton={false}
                  role="link"
                  render={
                    <Link
                      params={groupTarget}
                      to="/groups/$axisKey/$groupKey"
                    />
                  }
                  size="lg"
                  variant="outline"
                >
                  分類ページを開く
                </Button>
              )}
              <Button
                aria-label="詳細を閉じる"
                onClick={() => {
                  setSelectedRowKey(null)
                }}
                type="button"
                variant="plain"
              >
                閉じる
              </Button>
            </div>
          </div>
        </section>
      )}

      {model.rows.length > 0 && (
        <section className="border border-border bg-card">
          <Button
            aria-expanded={listExpanded}
            className="w-full justify-start"
            onClick={() => {
              setListExpanded((expanded) => !expanded)
            }}
            type="button"
            variant="ghost"
          >
            全件一覧 ({model.rows.length})
            <ChevronDown
              aria-hidden="true"
              className="ml-auto size-4 transition-transform data-[expanded=true]:rotate-180"
              data-expanded={String(listExpanded)}
            />
          </Button>
          {listExpanded && (
            <div className="border-t border-border">
              {model.groups.length > 0 ? (
                <div className="divide-y divide-border">
                  {model.groups.map((group) => (
                    <section key={group.id}>
                      <div className="flex min-h-10 items-start justify-between gap-3 bg-bg-secondary px-3 py-2">
                        <h3 className="min-w-0 flex-1 break-words text-xs font-semibold text-foreground">
                          {group.name}
                        </h3>
                        <Button
                          aria-pressed={focusId === group.id}
                          className="shrink-0"
                          disabled={group.area <= 0}
                          onClick={() => {
                            focusGroup(group.id)
                          }}
                          type="button"
                          variant="ghost"
                        >
                          分類にフォーカス
                        </Button>
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
          )}
        </section>
      )}
    </div>
  )
}
