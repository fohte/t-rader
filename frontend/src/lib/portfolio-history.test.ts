import { describe, expect, it } from 'vitest'

import {
  type PortfolioHistoryPeriod,
  type PortfolioHistoryPoint,
  selectPortfolioHistoryPoints,
} from '#lib/portfolio-history'

const points = {
  dec29: {
    date: '2025-12-29',
    total_assets: 1_010_000,
    stock_market_value: 610_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  dec31: {
    date: '2025-12-31',
    total_assets: 1_020_000,
    stock_market_value: 620_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  jan01: {
    date: '2026-01-01',
    total_assets: 1_030_000,
    stock_market_value: 630_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  jan04: {
    date: '2026-01-04',
    total_assets: 1_040_000,
    stock_market_value: 640_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  jan05: {
    date: '2026-01-05',
    total_assets: 1_050_000,
    stock_market_value: 650_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  jan31: {
    date: '2026-01-31',
    total_assets: 1_060_000,
    stock_market_value: 660_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  feb01: {
    date: '2026-02-01',
    total_assets: 1_070_000,
    stock_market_value: 670_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  dec31NextYear: {
    date: '2026-12-31',
    total_assets: 1_080_000,
    stock_market_value: 680_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  jan01NextYear: {
    date: '2027-01-01',
    total_assets: 1_090_000,
    stock_market_value: 690_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
} satisfies Record<string, PortfolioHistoryPoint>

const dailyPoints = Object.values(points)
const cases: {
  period: PortfolioHistoryPeriod
  expected: PortfolioHistoryPoint[]
}[] = [
  {
    period: 'week',
    expected: [points.jan04, points.jan05, points.feb01, points.jan01NextYear],
  },
  {
    period: 'month',
    expected: [
      points.dec31,
      points.jan31,
      points.feb01,
      points.dec31NextYear,
      points.jan01NextYear,
    ],
  },
  {
    period: 'year',
    expected: [points.dec31, points.dec31NextYear, points.jan01NextYear],
  },
  {
    period: 'day',
    expected: dailyPoints,
  },
]

describe('selectPortfolioHistoryPoints', () => {
  it.each(cases)(
    'keeps the last daily point for each $period period',
    ({ period, expected }) => {
      expect(selectPortfolioHistoryPoints(dailyPoints, period)).toEqual(
        expected,
      )
    },
  )
})
