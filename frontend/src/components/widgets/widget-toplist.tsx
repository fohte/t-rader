import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetToplistDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'toplist' }
>

const numberFormat = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

export function WidgetToplist({
  result,
  viz,
}: {
  result: WidgetQueryResponse
  viz: WidgetToplistDefinition
}) {
  const rows = result.rows.flatMap((row) => {
    const value = row.values[viz.value]
    return typeof value === 'number' && Number.isFinite(value)
      ? [{ row, value }]
      : []
  })
  const orderedRows = rows
    .sort((left, right) => {
      const valueOrder =
        viz.order === 'asc'
          ? left.value - right.value
          : right.value - left.value
      return valueOrder || left.row.key.localeCompare(right.row.key)
    })
    .slice(0, Math.max(0, viz.limit))

  if (orderedRows.length === 0) {
    return (
      <p
        className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
        role="status"
      >
        表示できるデータがありません。
      </p>
    )
  }

  return (
    <ol
      aria-label="ウィジェットの toplist"
      className="divide-y divide-border border border-border bg-card font-mono text-xs"
    >
      {orderedRows.map(({ row, value }, index) => (
        <li
          aria-label={`${String(index + 1)} 位: ${row.name} ${numberFormat.format(value)}`}
          className="flex items-center gap-3 px-3 py-2"
          key={row.key}
        >
          <span className="w-8 shrink-0 text-right text-muted-foreground">
            {index + 1}
          </span>
          <span className="min-w-0 flex-1 truncate text-foreground">
            {row.name}
          </span>
          <span className="shrink-0 tabular-nums text-muted-foreground-strong">
            {numberFormat.format(value)}
          </span>
        </li>
      ))}
    </ol>
  )
}
