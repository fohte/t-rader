import {
  type ChartIndicatorConfig,
  getChartIndicatorName,
} from '#lib/chart-indicator-settings'

function getIndicatorLabel(indicator: ChartIndicatorConfig): string {
  const name = getChartIndicatorName(indicator)
  if (indicator.kind === 'bollinger') {
    return `${name} ${indicator.period.toString()} ${indicator.deviation.toString()}σ`
  }
  if (indicator.kind === 'macd') {
    return `${name} ${indicator.fastPeriod.toString()} ${indicator.slowPeriod.toString()} ${indicator.signalPeriod.toString()}`
  }
  return `${name} ${indicator.period.toString()}`
}

export function ChartIndicatorLegend({
  indicators,
  paneTops = [],
}: {
  indicators: ChartIndicatorConfig[]
  paneTops?: readonly number[]
}) {
  if (indicators.length === 0) return null

  const indicatorsByPane = new Map<number, ChartIndicatorConfig[]>()
  let lowerPaneIndex = 0
  for (const indicator of indicators) {
    const paneIndex =
      indicator.kind === 'rsi' || indicator.kind === 'macd'
        ? ++lowerPaneIndex
        : 0
    const paneIndicators = indicatorsByPane.get(paneIndex) ?? []
    paneIndicators.push(indicator)
    indicatorsByPane.set(paneIndex, paneIndicators)
  }

  return (
    <>
      {[...indicatorsByPane.entries()].map(([paneIndex, paneIndicators]) => (
        <ul
          key={paneIndex}
          aria-label={
            paneIndex === 0
              ? '表示中のチャート指標'
              : `下段 ${paneIndex.toString()} の表示中の指標`
          }
          className="pointer-events-none absolute left-2 right-2 z-10 flex max-w-full flex-wrap gap-x-3 gap-y-1 rounded-sm bg-background/80 px-2 py-1 font-mono text-xs text-foreground backdrop-blur-sm"
          role="list"
          style={{ top: `${((paneTops[paneIndex] ?? 0) + 8).toString()}px` }}
        >
          {paneIndicators.map((indicator) => (
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
      ))}
    </>
  )
}
