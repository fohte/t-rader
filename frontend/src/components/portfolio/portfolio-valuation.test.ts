import { describe, expect, it } from 'vitest'

import { calculatePortfolioValuation } from '#components/portfolio/portfolio-valuation'
import type { components } from '#lib/api/schema.gen'

type Position = components['schemas']['PositionSummary']

const pricedPositionA: Position = {
  symbol: 'demo-code',
  qty: 10,
  avg_cost: 100,
  cost_basis: 1000,
  current_price: 150,
  market_value: 1500,
  unrealized_pnl: 500,
  realized_pnl: 25,
}
const pricedPositionB: Position = {
  symbol: 'demo-code-2',
  qty: 5,
  avg_cost: 200,
  cost_basis: 1000,
  current_price: 180,
  market_value: 900,
  unrealized_pnl: -100,
  realized_pnl: -10,
}
const pricedPositions = [pricedPositionA, pricedPositionB]

describe('calculatePortfolioValuation', () => {
  it('calculates market value, cash, total assets, and unrealized return', () => {
    expect(calculatePortfolioValuation(pricedPositions, 5000, 200)).toEqual({
      costBasis: 2000,
      marketValue: 2400,
      unrealizedPnl: 400,
      unrealizedPnlRate: 20,
      cash: 3200,
      totalAssets: 5600,
      cashRatio: 57.14285714285714,
      investedRatio: 42.857142857142854,
      stats: [
        { label: '総資産 (時価)', value: '¥5,600' },
        {
          label: '評価額 (株式・時価)',
          value: '¥2,400',
          sub: '簿価 ¥2,000',
        },
        {
          label: '含み損益',
          value: '+¥400',
          sub: '+20.0%',
          cls: 'text-up',
        },
        { label: '現金', value: '¥3,200', sub: '57.1% 比率' },
        { label: '実現損益 (累計)', value: '+¥200', cls: 'text-up' },
      ],
    })
  })

  it('leaves aggregate market values unavailable when a position has no price', () => {
    const positions: Position[] = [
      pricedPositionA,
      {
        ...pricedPositionB,
        current_price: null,
        market_value: null,
        unrealized_pnl: null,
      },
    ]

    expect(calculatePortfolioValuation(positions, 5000, 200)).toEqual({
      costBasis: 2000,
      marketValue: null,
      unrealizedPnl: null,
      unrealizedPnlRate: null,
      cash: 3200,
      totalAssets: null,
      cashRatio: null,
      investedRatio: null,
      stats: [
        { label: '総資産 (時価)', value: '—' },
        {
          label: '評価額 (株式・時価)',
          value: '—',
          sub: '簿価 ¥2,000',
        },
        {
          label: '含み損益',
          value: '—',
          sub: '—',
          cls: 'text-muted-foreground-strong',
        },
        { label: '現金', value: '¥3,200', sub: '— 比率' },
        { label: '実現損益 (累計)', value: '+¥200', cls: 'text-up' },
      ],
    })
  })
})
