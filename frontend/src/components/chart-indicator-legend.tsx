import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

function getIndicatorLabel(indicator: ChartIndicatorConfig): string {
  if (indicator.kind === 'sma') return `SMA ${indicator.period.toString()}`
  if (indicator.kind === 'ema') return `EMA ${indicator.period.toString()}`
  return `ボリンジャー ${indicator.period.toString()} ${indicator.deviation.toString()}σ`
}

export function ChartIndicatorLegend({
  indicators,
}: {
  indicators: ChartIndicatorConfig[]
}) {
  if (indicators.length === 0) return null

  return (
    <ul
      aria-label="表示中のチャート指標"
      className="pointer-events-none absolute left-2 right-2 top-2 z-10 flex max-w-full flex-wrap gap-x-3 gap-y-1 rounded-sm bg-background/80 px-2 py-1 font-mono text-xs text-foreground backdrop-blur-sm"
      role="list"
    >
      {indicators.map((indicator) => (
        <li key={indicator.id} className="flex items-center gap-1.5">
          <span
            aria-hidden="true"
            className="h-0.5 w-3"
            style={{ backgroundColor: indicator.color }}
          />
          {getIndicatorLabel(indicator)}
        </li>
      ))}
    </ul>
  )
}
