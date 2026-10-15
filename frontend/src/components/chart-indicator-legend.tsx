import {
  type ChartIndicatorConfig,
  getChartIndicatorLabel,
  getChartIndicatorPaneIndexes,
} from '#lib/chart-indicator-settings'

function getIndicatorLabel(indicator: ChartIndicatorConfig): string {
  if (indicator.kind === 'bollinger') {
    return `${getChartIndicatorLabel(indicator)} ${indicator.deviation.toString()}σ`
  }
  return getChartIndicatorLabel(indicator)
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
  const paneIndexes = getChartIndicatorPaneIndexes(indicators)
  for (const [indicatorIndex, indicator] of indicators.entries()) {
    const paneIndex = paneIndexes[indicatorIndex] ?? 0
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
          className={`pointer-events-none absolute left-2 z-10 flex flex-wrap gap-x-3 gap-y-1 font-mono text-xs text-foreground ${
            paneIndex === 0
              ? 'right-2 max-w-full rounded-sm bg-background/80 px-2 py-1 backdrop-blur-sm'
              : 'w-fit max-w-full bg-transparent'
          }`}
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
