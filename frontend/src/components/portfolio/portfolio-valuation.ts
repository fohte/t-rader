import type { StatItem } from '#components/portfolio/stat-row'
import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type PositionSummary = components['schemas']['PositionSummary']

export interface PortfolioValuation {
  costBasis: number
  marketValue: number | null
  unrealizedPnl: number | null
  unrealizedPnlRate: number | null
  cash: number
  totalAssets: number | null
  cashRatio: number | null
  investedRatio: number | null
  stats: StatItem[]
}

export function calculatePortfolioValuation(
  positions: PositionSummary[],
  investableAmount: number,
  realizedPnl: number,
): PortfolioValuation {
  const costBasis = positions.reduce(
    (sum, position) => sum + position.cost_basis,
    0,
  )
  const marketValue = positions.some(
    (position) => position.market_value == null,
  )
    ? null
    : positions.reduce((sum, position) => sum + (position.market_value ?? 0), 0)
  const unrealizedPnl = positions.some(
    (position) => position.unrealized_pnl == null,
  )
    ? null
    : positions.reduce(
        (sum, position) => sum + (position.unrealized_pnl ?? 0),
        0,
      )
  const unrealizedPnlRate =
    unrealizedPnl != null && costBasis > 0
      ? (unrealizedPnl / costBasis) * 100
      : null
  const cash = investableAmount + realizedPnl - costBasis
  const totalAssets = marketValue == null ? null : cash + marketValue
  const cashRatio =
    totalAssets != null && totalAssets > 0 ? (cash / totalAssets) * 100 : null
  const investedRatio =
    totalAssets != null && totalAssets > 0 && marketValue != null
      ? (marketValue / totalAssets) * 100
      : null
  const stats: StatItem[] = [
    {
      label: '総資産 (時価)',
      value: totalAssets == null ? '—' : formatYen(totalAssets),
    },
    {
      label: '評価額 (株式・時価)',
      value: marketValue == null ? '—' : formatYen(marketValue),
      sub: `簿価 ${formatYen(costBasis)}`,
    },
    {
      label: '含み損益',
      value: unrealizedPnl == null ? '—' : formatYen(unrealizedPnl, true),
      sub:
        unrealizedPnlRate == null
          ? '—'
          : `${unrealizedPnlRate > 0 ? '+' : ''}${unrealizedPnlRate.toFixed(1)}%`,
      cls:
        unrealizedPnl == null
          ? 'text-muted-foreground-strong'
          : pnlColorClass(unrealizedPnl),
    },
    {
      label: '現金',
      value: formatYen(cash),
      sub: cashRatio == null ? '— 比率' : `${cashRatio.toFixed(1)}% 比率`,
    },
    {
      label: '実現損益 (累計)',
      value: formatYen(realizedPnl, true),
      cls: pnlColorClass(realizedPnl),
    },
  ]

  return {
    costBasis,
    marketValue,
    unrealizedPnl,
    unrealizedPnlRate,
    cash,
    totalAssets,
    cashRatio,
    investedRatio,
    stats,
  }
}
