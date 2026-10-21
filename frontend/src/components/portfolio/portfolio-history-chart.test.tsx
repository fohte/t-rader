import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { PortfolioHistoryChart } from '#components/portfolio/portfolio-history-chart'
import type {
  PortfolioHistoryPeriod,
  PortfolioHistoryPoint,
} from '#lib/portfolio-history'

const chartMocks = vi.hoisted(() => {
  const series = Array.from({ length: 3 }, () => ({
    applyOptions: vi.fn(),
    setData: vi.fn(),
  }))
  let nextSeriesIndex = 0
  const timeScale = { fitContent: vi.fn() }
  const chart = {
    addSeries: vi.fn(() => series[nextSeriesIndex++]),
    applyOptions: vi.fn(),
    remove: vi.fn(),
    subscribeCrosshairMove: vi.fn(),
    timeScale: vi.fn(() => timeScale),
    unsubscribeCrosshairMove: vi.fn(),
  }

  return {
    chart,
    series,
    resetSeriesIndex: () => {
      nextSeriesIndex = 0
    },
  }
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

const originalRootClass = document.documentElement.className
const originalRootStyle = document.documentElement.getAttribute('style')

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

function readThemeUpdateCalls() {
  return [
    chartMocks.chart.applyOptions.mock.calls,
    ...chartMocks.series.map((series) => series.applyOptions.mock.calls),
  ]
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
  chartMocks.resetSeriesIndex()
})

afterEach(() => {
  cleanup()
  document.documentElement.className = originalRootClass
  if (originalRootStyle == null) {
    document.documentElement.removeAttribute('style')
  } else {
    document.documentElement.setAttribute('style', originalRootStyle)
  }
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

  it('updates series colors when the theme tokens change', async () => {
    const root = document.documentElement
    root.classList.remove('dark')
    root.style.setProperty('--chart-1', 'oklch(0 0 0)')
    root.style.setProperty('--chart-2', 'oklch(0 0 0)')
    root.style.setProperty('--chart-3', 'oklch(0 0 0)')
    root.style.setProperty('--foreground', 'oklch(0 0 0)')

    render(
      <PortfolioHistoryChart
        data={history}
        period="week"
        status="ready"
        onPeriodChange={vi.fn()}
      />,
    )

    chartMocks.chart.applyOptions.mockClear()
    for (const series of chartMocks.series) series.applyOptions.mockClear()
    root.style.setProperty('--chart-1', 'oklch(1 0 0)')
    root.style.setProperty('--chart-2', 'oklch(0 0 0)')
    root.style.setProperty('--chart-3', 'oklch(0.5 0 0)')
    root.style.setProperty('--foreground', 'oklch(0.5 0 0)')
    root.classList.add('dark')

    await waitFor(() => {
      expect(readThemeUpdateCalls()).toEqual([
        [
          [
            {
              layout: {
                background: { type: 'solid', color: '#1a1a1a' },
                textColor: '#d1d5db',
              },
              grid: {
                vertLines: { color: '#2d2d2d' },
                horzLines: { color: '#2d2d2d' },
              },
              timeScale: { borderColor: '#3f3f46' },
              rightPriceScale: { borderColor: '#3f3f46' },
            },
          ],
        ],
        [
          [
            {
              lineColor: 'rgb(99, 99, 99)',
              topColor: 'rgba(0, 0, 0, 0.32)',
              bottomColor: 'rgba(0, 0, 0, 0.08)',
            },
          ],
        ],
        [
          [
            {
              lineColor: 'rgb(255, 255, 255)',
              topColor: 'rgba(255, 255, 255, 0.5)',
              bottomColor: 'rgba(255, 255, 255, 0.24)',
            },
          ],
        ],
        [[{ color: 'rgb(99, 99, 99)' }]],
      ])
    })
  })
})
