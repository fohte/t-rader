import '#lib/echarts/bar'

import { Link } from '@tanstack/react-router'
import type { ECElementEvent } from 'echarts/core'
import { useCallback, useMemo, useState } from 'react'

import {
  createWidgetToplistModel,
  createWidgetToplistOption,
  getWidgetToplistColor,
} from '#components/widgets/widget-toplist-model'
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
type WidgetToplistDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'toplist' }
>

const EMPTY_DISPLAY: WidgetDisplayDefinitions = {}
const CHART_ROW_HEIGHT = 58
const CHART_COLUMNS = '134px minmax(24px, 1fr) 104px'

export function WidgetToplist({
  result,
  viz,
  queries,
  display,
  initialSelectedRowKey,
}: {
  result: WidgetQueryResponse
  viz: WidgetToplistDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
  initialSelectedRowKey?: string
}) {
  const displayDefinitions = display ?? EMPTY_DISPLAY
  const model = useMemo(
    () =>
      createWidgetToplistModel({
        result,
        viz,
        queries,
        display: displayDefinitions,
      }),
    [displayDefinitions, queries, result, viz],
  )
  const [selectedRowKey, setSelectedRowKey] = useState<string | null>(
    initialSelectedRowKey ?? null,
  )
  const option = useMemo(
    () => (palette: EChartsPalette) =>
      createWidgetToplistOption(model, palette),
    [model],
  )
  const handleChartClick = useCallback(
    (event: ECElementEvent) => {
      if (event.componentType !== 'series' || event.seriesType !== 'bar') {
        return
      }
      const row = model.rows[event.dataIndex]
      if (row != null) setSelectedRowKey(row.key)
    },
    [model.rows],
  )
  const { containerRef, error, palette } = useEChartsChart({
    createOption: option,
    onClick: handleChartClick,
  })

  const selectedRow =
    selectedRowKey == null
      ? null
      : (model.rows.find((row) => row.key === selectedRowKey) ?? null)
  const selectedRank =
    selectedRow == null
      ? null
      : model.rows.findIndex((row) => row.key === selectedRow.key) + 1
  const stockId =
    selectedRow?.key.startsWith('stock:') === true
      ? selectedRow.key.slice('stock:'.length)
      : null
  const indicatorId =
    selectedRow?.key.startsWith('indicator:') === true
      ? selectedRow.key.slice('indicator:'.length)
      : null
  const groupTarget =
    selectedRow?.key.startsWith('group:') === true
      ? parseGroupRefId(selectedRow.key.slice('group:'.length))
      : null
  const scaleMin = formatWidgetValue(
    viz.value,
    model.scale.min,
    queries,
    displayDefinitions,
  )
  const scaleMax = formatWidgetValue(
    viz.value,
    model.scale.max,
    queries,
    displayDefinitions,
  )

  const isEmpty = model.rows.length === 0
  const chartHeight = Math.max(112, model.rows.length * CHART_ROW_HEIGHT)
  const chartRows = `repeat(${String(model.rows.length)}, minmax(0, 1fr))`

  return (
    <div className="space-y-3">
      {isEmpty ? (
        <p
          className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
          role="status"
        >
          表示できるデータがありません。
        </p>
      ) : (
        <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span>{model.valueLabel}</span>
          <span className="font-mono">{result.as_of}</span>
          <span className="font-mono tabular-nums">
            {model.scale.configured ? '指定尺度' : '自動尺度'}: {scaleMin} / 0 /{' '}
            {scaleMax}
          </span>
        </div>
      )}
      <div
        className="w-full overflow-hidden border border-border bg-card"
        hidden={isEmpty}
      >
        <div
          className="grid w-full"
          style={{ gridTemplateColumns: CHART_COLUMNS, height: chartHeight }}
        >
          <ol
            aria-hidden="true"
            className="relative z-10 m-0 grid min-w-0 list-none bg-card p-0"
            data-column="identity"
            style={{ gridTemplateRows: chartRows }}
          >
            {model.rows.map((row, index) => (
              <li
                className="flex min-h-0 min-w-0 items-center border-b border-border px-2 last:border-b-0"
                key={row.key}
              >
                <div className="flex w-full min-w-0 items-center gap-2">
                  <span className="w-4 shrink-0 text-right font-mono text-2xs text-muted-foreground">
                    {index + 1}
                  </span>
                  <span className="min-w-0">
                    <span className="block truncate text-xs text-foreground">
                      {row.name}
                    </span>
                    <span className="block truncate font-mono text-2xs text-muted-foreground">
                      {row.code}
                    </span>
                  </span>
                </div>
              </li>
            ))}
          </ol>
          <div
            aria-label={model.ariaDescription}
            className="min-w-0"
            data-column="bars"
            ref={containerRef}
            role="img"
            style={{ height: chartHeight }}
          />
          <ol
            aria-hidden="true"
            className="m-0 grid min-w-0 list-none p-0"
            data-column="values"
            style={{ gridTemplateRows: chartRows }}
          >
            {model.rows.map((row) => (
              <li
                className="flex min-h-0 items-center justify-end whitespace-nowrap border-b border-border px-2 text-right font-mono tabular-nums text-xs last:border-b-0"
                key={row.key}
                style={{
                  color: getWidgetToplistColor(
                    row.value,
                    palette,
                    palette.foreground,
                  ),
                }}
              >
                {row.formattedValue}
              </li>
            ))}
          </ol>
        </div>
      </div>
      {!isEmpty && error != null && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}

      {!isEmpty && selectedRow != null && (
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
                <dt className="text-muted-foreground">順位</dt>
                <dd className="text-right font-mono tabular-nums text-foreground">
                  {selectedRank}
                </dd>
                <dt className="text-muted-foreground">コード</dt>
                <dd className="text-right font-mono text-foreground">
                  {selectedRow.code}
                </dd>
                <dt className="text-muted-foreground">{model.valueLabel}</dt>
                <dd className="text-right font-mono tabular-nums text-foreground">
                  {selectedRow.formattedValue}
                </dd>
                <dt className="text-muted-foreground">基準日</dt>
                <dd className="text-right font-mono text-foreground">
                  {result.as_of}
                </dd>
              </dl>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {stockId != null && (
                <Link
                  className="min-h-9 border border-border px-3 py-2 text-xs text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                  params={{ stockId }}
                  to="/stocks/$stockId"
                >
                  銘柄ページを開く
                </Link>
              )}
              {indicatorId != null && (
                <Link
                  className="min-h-9 border border-border px-3 py-2 text-xs text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                  params={{ indicatorId }}
                  to="/indicators/$indicatorId"
                >
                  指標ページを開く
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

      {!isEmpty && (
        <details className="border border-border bg-card">
          <summary className="min-h-10 cursor-pointer px-3 py-2 text-xs text-foreground focus-visible:outline-2 focus-visible:outline-ring">
            {`順位一覧から詳細を選ぶ (${String(model.rows.length)})`}
          </summary>
          <ol
            aria-label="toplist の順位"
            className="list-none border-t border-border"
          >
            {model.rows.map((row, index) => (
              <li
                className="border-t border-border first:border-t-0"
                key={row.key}
              >
                <button
                  aria-label={
                    String(index + 1) +
                    ' 位: ' +
                    row.name +
                    '、' +
                    row.code +
                    '、' +
                    row.formattedValue +
                    '、詳細を表示'
                  }
                  className="flex min-h-12 w-full items-center gap-3 px-3 py-2 text-left hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
                  onClick={() => {
                    setSelectedRowKey(row.key)
                  }}
                  type="button"
                >
                  <span className="w-8 shrink-0 text-right font-mono text-muted-foreground">
                    {index + 1}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block break-words text-foreground">
                      {row.name}
                    </span>
                    <span className="block break-all font-mono text-2xs text-muted-foreground">
                      {row.code}
                    </span>
                  </span>
                  <span className="shrink-0 font-mono tabular-nums text-muted-foreground-strong">
                    {row.formattedValue}
                  </span>
                </button>
              </li>
            ))}
          </ol>
        </details>
      )}
    </div>
  )
}
