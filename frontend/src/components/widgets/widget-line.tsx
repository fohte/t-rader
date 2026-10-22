import '#lib/echarts/line'

import { useMemo } from 'react'

import {
  createWidgetLineModel,
  createWidgetLineOption,
} from '#components/widgets/widget-line-model'
import type { components } from '#lib/api/schema.gen'
import {
  type EChartsPalette,
  useEChartsChart,
} from '#lib/echarts/use-echarts-chart'
import type {
  WidgetDisplayDefinitions,
  WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetLineDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'line' }
>

const EMPTY_DISPLAY: WidgetDisplayDefinitions = {}

export function WidgetLine({
  result,
  viz,
  queries,
  display,
}: {
  result: WidgetQueryResponse
  viz: WidgetLineDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
}) {
  const displayDefinitions = display ?? EMPTY_DISPLAY
  const model = useMemo(
    () =>
      createWidgetLineModel({
        result,
        viz,
        queries,
        display: displayDefinitions,
      }),
    [displayDefinitions, queries, result, viz],
  )
  const option = useMemo(
    () => (palette: EChartsPalette) =>
      createWidgetLineOption(model, queries, displayDefinitions, palette),
    [displayDefinitions, model, queries],
  )
  const { containerRef, error } = useEChartsChart({ createOption: option })

  if (!model.hasValues) {
    return (
      <p
        className="border border-dashed border-border px-4 py-8 text-center font-mono text-xs text-muted-foreground"
        role="status"
      >
        表示できる値がありません。
      </p>
    )
  }

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 font-mono text-2xs text-muted-foreground">
        <span>{String(model.series.length)} 系列</span>
        <span>
          {model.startDate} – {model.endDate}
        </span>
      </div>
      <div
        aria-label={model.ariaDescription}
        className="h-72 min-w-0 w-full border border-border bg-card"
        ref={containerRef}
        role="img"
      />
      {error != null && (
        <p className="text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}
