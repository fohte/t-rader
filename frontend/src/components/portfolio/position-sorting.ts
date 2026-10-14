import type { components } from '#lib/api/schema.gen'

type PositionSummary = components['schemas']['PositionSummary']

export function sortPositionsByMarketValue(
  positions: PositionSummary[],
): PositionSummary[] {
  return [...positions].sort((a, b) => {
    if (a.market_value == null) {
      return b.market_value == null ? a.symbol.localeCompare(b.symbol) : 1
    }
    if (b.market_value == null) return -1
    return b.market_value - a.market_value || a.symbol.localeCompare(b.symbol)
  })
}
