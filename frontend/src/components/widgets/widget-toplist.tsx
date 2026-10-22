import '#lib/echarts/bar'

import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'
import type { ECElementEvent } from 'echarts/core'
import { useCallback, useMemo, useState } from 'react'

import { WidgetCollapsibleSection } from '#components/widgets/widget-collapsible-section'
import type { WidgetCSSProperties } from '#components/widgets/widget-css'
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
const DETAIL_STYLE: WidgetCSSProperties = {
  '--widget-detail-columns': 'auto 1fr',
}

export function WidgetToplist({
  result,
  viz,
  queries,
  display,
  initialSelectedRowKey,
  initialRankListExpanded,
}: {
  result: WidgetQueryResponse
  viz: WidgetToplistDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
  initialSelectedRowKey?: string
  initialRankListExpanded?: boolean
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
  const [rankListExpanded, setRankListExpanded] = useState(
    initialRankListExpanded ?? false,
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
  const chartStyle: WidgetCSSProperties = {
    '--widget-chart-height': String(chartHeight) + 'px',
    '--widget-chart-rows': `repeat(${String(model.rows.length)}, minmax(0, 1fr))`,
    '--widget-chart-columns': '134px minmax(24px, 1fr) 104px',
  }

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
        style={chartStyle}
      >
        <div className="grid h-(--widget-chart-height) w-full grid-cols-(--widget-chart-columns)">
          <ol
            aria-hidden="true"
            className="relative z-10 m-0 grid min-w-0 list-none bg-card p-0 grid-rows-(--widget-chart-rows)"
            data-column="identity"
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
            className="h-(--widget-chart-height) min-w-0"
            data-column="bars"
            ref={containerRef}
            role="img"
          />
          <ol
            aria-hidden="true"
            className="m-0 grid min-w-0 list-none p-0 grid-rows-(--widget-chart-rows)"
            data-column="values"
          >
            {model.rows.map((row) => {
              const valueStyle: WidgetCSSProperties = {
                '--widget-value-color': getWidgetToplistColor(
                  row.value,
                  palette,
                  palette.foreground,
                ),
              }
              return (
                <li
                  className="flex min-h-0 items-center justify-end whitespace-nowrap border-b border-border px-2 text-right font-mono tabular-nums text-xs last:border-b-0 text-(color:--widget-value-color)"
                  key={row.key}
                  style={valueStyle}
                >
                  {row.formattedValue}
                </li>
              )
            })}
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
                className="mt-2 grid grid-cols-(--widget-detail-columns) gap-x-4 gap-y-1 text-xs"
                style={DETAIL_STYLE}
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
              {indicatorId != null && (
                <Button
                  nativeButton={false}
                  role="link"
                  render={
                    <Link
                      params={{ indicatorId }}
                      to="/indicators/$indicatorId"
                    />
                  }
                  size="lg"
                  variant="outline"
                >
                  指標ページを開く
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

      {!isEmpty && (
        <WidgetCollapsibleSection
          expanded={rankListExpanded}
          label={`順位一覧から詳細を選ぶ (${String(model.rows.length)})`}
          onExpandedChange={setRankListExpanded}
        >
          <ol aria-label="toplist の順位" className="list-none">
            {model.rows.map((row, index) => (
              <li
                className="border-t border-border first:border-t-0"
                key={row.key}
              >
                <Button
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
                  className="min-h-12 w-full justify-start whitespace-normal text-left"
                  onClick={() => {
                    setSelectedRowKey(row.key)
                  }}
                  size="lg"
                  type="button"
                  variant="ghost"
                >
                  <span className="w-8 shrink-0 text-right text-muted-foreground">
                    {index + 1}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block break-words text-foreground">
                      {row.name}
                    </span>
                    <span className="block break-all text-2xs text-muted-foreground">
                      {row.code}
                    </span>
                  </span>
                  <span className="shrink-0 tabular-nums text-muted-foreground-strong">
                    {row.formattedValue}
                  </span>
                </Button>
              </li>
            ))}
          </ol>
        </WidgetCollapsibleSection>
      )}
    </div>
  )
}
