import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'
import type {
  PortfolioHistoryPeriod,
  PortfolioHistoryPoint,
} from '#lib/portfolio-history'

const chartMocks = vi.hoisted(() => {
  const series = { applyOptions: vi.fn(), setData: vi.fn() }
  const timeScale = { fitContent: vi.fn() }
  const chart = {
    addSeries: vi.fn(() => series),
    applyOptions: vi.fn(),
    remove: vi.fn(),
    subscribeCrosshairMove: vi.fn(),
    timeScale: vi.fn(() => timeScale),
    unsubscribeCrosshairMove: vi.fn(),
  }

  return { chart }
})

vi.mock('lightweight-charts', () => ({
  AreaSeries: {},
  ColorType: { Solid: 'solid' },
  createChart: vi.fn(() => chartMocks.chart),
  LineSeries: {},
  LineStyle: { Dashed: 2 },
}))

const history: PortfolioHistoryPoint[] = [
  {
    date: '2025-04-07',
    total_assets: 1_000_000,
    stock_market_value: 600_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2025-04-11',
    total_assets: 1_010_000,
    stock_market_value: 610_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2025-04-14',
    total_assets: 1_020_000,
    stock_market_value: 620_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
  {
    date: '2025-04-18',
    total_assets: 1_015_000,
    stock_market_value: 615_000,
    cash: 400_000,
    investable_amount: 1_000_000,
  },
]

function readTooltip() {
  const tooltip = screen.queryByRole('tooltip')
  if (tooltip == null) return null

  return {
    date: tooltip.querySelector('p')?.textContent ?? null,
    metrics: Array.from(tooltip.querySelectorAll('dl > div')).map((row) => ({
      label: row.querySelector('dt')?.textContent ?? null,
      value: row.querySelector('dd')?.textContent ?? null,
    })),
  }
}

function StatefulHistoryChart() {
  const [period, setPeriod] = useState<PortfolioHistoryPeriod>('week')

  return (
    <PortfolioHistoryChart
      data={history}
      period={period}
      status="ready"
      initialHoveredDate="2025-04-18"
      onPeriodChange={setPeriod}
    />
  )
}

beforeEach(() => {
  vi.clearAllMocks()
})

afterEach(() => {
  cleanup()
})

describe('PortfolioHistoryChart', () => {
  it('notifies when the selected period changes', async () => {
    const user = userEvent.setup()
    const onPeriodChange = vi.fn()

    render(
      <PortfolioHistoryChart
        data={[]}
        period="month"
        status="empty"
        onPeriodChange={onPeriodChange}
      />,
    )

    await user.click(screen.getByRole('button', { name: '週' }))

    expect(onPeriodChange.mock.calls).toEqual([['week']])
  })

  it('clears the hovered point when changing the displayed period', async () => {
    const user = userEvent.setup()

    render(<StatefulHistoryChart />)

    const tooltipSnapshots = [readTooltip()]

    await user.click(screen.getByRole('button', { name: '月' }))
    tooltipSnapshots.push(readTooltip())

    await user.click(screen.getByRole('button', { name: '週' }))
    tooltipSnapshots.push(readTooltip())

    expect(tooltipSnapshots).toEqual([
      {
        date: '2025-04-18',
        metrics: [
          { label: '総資産', value: '¥1,015,000' },
          { label: '株式 (時価)', value: '¥615,000' },
          { label: '現金', value: '¥400,000' },
          { label: '前期比', value: '+¥5,000' },
        ],
      },
      null,
      null,
    ])
  })
})
