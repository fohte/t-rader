import { cleanup, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CandlestickChart } from '#components/candlestick-chart'
import type { components } from '#lib/api/schema.gen'

const chartMocks = vi.hoisted(() => {
  const candlePriceScale = { applyOptions: vi.fn() }
  const volumePriceScale = {
    applyOptions:
      vi.fn<
        (options: { scaleMargins: { top: number; bottom: number } }) => void
      >(),
  }
  const candlestickSeries = {
    applyOptions: vi.fn(),
    createPriceLine: vi.fn(),
    priceScale: vi.fn(() => candlePriceScale),
    removePriceLine: vi.fn(),
    setData: vi.fn(),
  }
  const volumeSeries = {
    priceScale: vi.fn(() => volumePriceScale),
    setData: vi.fn(),
  }
  const timeScale = {
    fitContent: vi.fn(),
    height: vi.fn(() => 24),
    subscribeSizeChange: vi.fn(),
    subscribeVisibleTimeRangeChange: vi.fn(),
    timeToCoordinate: vi.fn(() => 36),
    unsubscribeSizeChange: vi.fn(),
    unsubscribeVisibleTimeRangeChange: vi.fn(),
    width: vi.fn(() => 120),
  }
  const chart = {
    addSeries: vi.fn((): unknown => null),
    applyOptions: vi.fn(),
    remove: vi.fn(),
    timeScale: vi.fn(() => timeScale),
  }
  chart.addSeries.mockImplementation(() =>
    chart.addSeries.mock.calls.length === 1 ? candlestickSeries : volumeSeries,
  )

  return { chart, volumePriceScale }
})

vi.mock('lightweight-charts', () => ({
  CandlestickSeries: {},
  ColorType: { Solid: 'solid' },
  createChart: vi.fn(() => chartMocks.chart),
  HistogramSeries: {},
  LineStyle: { Dashed: 2 },
}))

const bars: components['schemas']['Bar'][] = [
  {
    close: 101,
    high: 102,
    instrument_id: 'demo-code',
    low: 99,
    open: 100,
    timeframe: '1d',
    timestamp: '2025-04-10T00:00:00.000Z',
    volume: 42,
  },
]

const annotations: NonNullable<
  ComponentProps<typeof CandlestickChart>['annotations']
> = [
  {
    id: 'annotation-a',
    price: 100,
    status: 'unread',
    target_kind: '架空分類',
    text: '架空の注釈',
    timestamp: '2025-04-10T00:00:00.000Z',
  },
]

beforeEach(() => {
  vi.clearAllMocks()
})

afterEach(() => {
  cleanup()
})

describe('CandlestickChart', () => {
  it('adds the annotation band and its spacing when annotations arrive', () => {
    const { rerender } = render(
      <CandlestickChart bars={bars} annotations={[]} />,
    )

    rerender(<CandlestickChart bars={bars} annotations={annotations} />)

    const readRenderedState = () => ({
      band: {
        label: screen
          .getByRole('group', { name: 'チャートのアノテーション' })
          .getAttribute('aria-label'),
        markerCount: screen
          .getByRole('group', { name: 'チャートのアノテーション' })
          .querySelectorAll('button').length,
      },
      volumeScaleMargins:
        chartMocks.volumePriceScale.applyOptions.mock.calls.map(
          ([options]) => options.scaleMargins,
        ),
    })

    expect(readRenderedState()).toEqual({
      band: { label: 'チャートのアノテーション', markerCount: 1 },
      volumeScaleMargins: [
        { top: 0.8, bottom: 0 },
        { top: 0.8, bottom: 0.06 },
      ],
    })
  })
})
