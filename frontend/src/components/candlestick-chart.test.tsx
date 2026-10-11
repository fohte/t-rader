import { cleanup, render, screen } from '@testing-library/react'
import type { CreatePriceLineOptions, UTCTimestamp } from 'lightweight-charts'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CandlestickChart } from '#components/candlestick-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
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
    createPriceLine: vi.fn<(options: CreatePriceLineOptions) => object>(
      () => ({}),
    ),
    priceScale: vi.fn(() => candlePriceScale),
    removePriceLine: vi.fn(),
    setData: vi.fn(),
  }
  const annotationPriceSeries = {
    setData: vi.fn<(data: { time: UTCTimestamp; value: number }[]) => void>(),
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
  chart.addSeries.mockImplementation(() => {
    const index = chart.addSeries.mock.calls.length
    if (index === 1) return candlestickSeries
    if (index === 2) return annotationPriceSeries
    return volumeSeries
  })

  return {
    chart,
    candlestickSeries,
    annotationPriceSeries,
    volumePriceScale,
  }
})

vi.mock('lightweight-charts', () => ({
  CandlestickSeries: {},
  ColorType: { Solid: 'solid' },
  createChart: vi.fn(() => chartMocks.chart),
  HistogramSeries: {},
  LineSeries: {},
  LineStyle: { Dashed: 2 },
}))

const firstBar: components['schemas']['Bar'] = {
  close: 101,
  high: 102,
  instrument_id: 'demo-code',
  low: 99,
  open: 100,
  timeframe: '1d',
  timestamp: '2025-04-10T00:00:00.000Z',
  volume: 42,
}

const bars: components['schemas']['Bar'][] = [firstBar]

const pointAnnotation: ChartAnnotation = {
  id: 'annotation-a',
  price: 100,
  status: 'unread',
  target_kind: '架空分類',
  text: '架空の注釈',
  timestamp: '2025-04-10T00:00:00.000Z',
}

const annotations: ChartAnnotation[] = [pointAnnotation]

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

  it('shows a selected period price only across its interval and keeps its axis label', () => {
    const periodAnnotation = {
      ...pointAnnotation,
      timestamp_start: '2025-04-10T00:00:00.000Z',
      timestamp: '2025-04-11T00:00:00.000Z',
    }
    const periodBars = [
      firstBar,
      { ...firstBar, timestamp: '2025-04-11T00:00:00.000Z' },
    ]

    render(
      <CandlestickChart
        bars={periodBars}
        annotations={[periodAnnotation]}
        selectedAnnotationId={periodAnnotation.id}
      />,
    )

    const readRenderedState = () => ({
      priceLines: chartMocks.candlestickSeries.createPriceLine.mock.calls.map(
        ([options]) => options,
      ),
      periodLineData: chartMocks.annotationPriceSeries.setData.mock.calls,
    })

    expect(readRenderedState()).toEqual({
      priceLines: [
        {
          price: 100,
          color: '#ef4444',
          lineWidth: 2,
          lineStyle: 2,
          lineVisible: false,
          axisLabelVisible: true,
          title: '架空分類',
          axisLabelColor: '#ef4444',
          axisLabelTextColor: '#ffffff',
        },
      ],
      periodLineData: [
        [[]],
        [
          [
            { time: 1744243200, value: 100 },
            { time: 1744329600, value: 100 },
          ],
        ],
      ],
    })
  })

  it('keeps the price axis label for a same-bucket period on a one-bar chart', () => {
    const periodAnnotation = {
      ...pointAnnotation,
      timestamp_start: '2025-04-10T03:00:00.000Z',
      timestamp: '2025-04-10T12:00:00.000Z',
    }

    render(
      <CandlestickChart
        bars={bars}
        annotations={[periodAnnotation]}
        selectedAnnotationId={periodAnnotation.id}
      />,
    )

    const readRenderedState = () => ({
      priceLines: chartMocks.candlestickSeries.createPriceLine.mock.calls.map(
        ([options]) => options,
      ),
      periodLineData: chartMocks.annotationPriceSeries.setData.mock.calls,
    })

    expect(readRenderedState()).toEqual({
      priceLines: [
        {
          price: 100,
          color: '#ef4444',
          lineWidth: 2,
          lineStyle: 2,
          lineVisible: false,
          axisLabelVisible: true,
          title: '架空分類',
          axisLabelColor: '#ef4444',
          axisLabelTextColor: '#ffffff',
        },
      ],
      periodLineData: [[[]]],
    })
  })

  it('extends a same-bucket period to an adjacent bar', () => {
    const periodAnnotation = {
      ...pointAnnotation,
      timestamp_start: '2025-04-10T00:00:00.000Z',
      timestamp: '2025-04-10T12:00:00.000Z',
    }
    const periodBars = [
      firstBar,
      { ...firstBar, timestamp: '2025-04-11T00:00:00.000Z' },
    ]

    render(
      <CandlestickChart
        bars={periodBars}
        annotations={[periodAnnotation]}
        selectedAnnotationId={periodAnnotation.id}
      />,
    )

    const readRenderedState = () => ({
      periodLineData: chartMocks.annotationPriceSeries.setData.mock.calls,
    })

    expect(readRenderedState()).toEqual({
      periodLineData: [
        [[]],
        [
          [
            { time: 1744243200, value: 100 },
            { time: 1744329600, value: 100 },
          ],
        ],
      ],
    })
  })

  it('clears the selected period price when selection changes', () => {
    const periodAnnotation = {
      ...pointAnnotation,
      timestamp_start: '2025-04-10T00:00:00.000Z',
      timestamp: '2025-04-11T00:00:00.000Z',
    }
    const periodBars = [
      firstBar,
      { ...firstBar, timestamp: '2025-04-11T00:00:00.000Z' },
    ]
    const { rerender } = render(
      <CandlestickChart
        bars={periodBars}
        annotations={[periodAnnotation]}
        selectedAnnotationId={periodAnnotation.id}
      />,
    )

    rerender(
      <CandlestickChart
        bars={periodBars}
        annotations={[pointAnnotation]}
        selectedAnnotationId={pointAnnotation.id}
      />,
    )

    const readSelectedState = () => ({
      periodLineData: chartMocks.annotationPriceSeries.setData.mock.calls.map(
        ([data]) => data,
      ),
      priceLines: chartMocks.candlestickSeries.createPriceLine.mock.calls.map(
        ([options]) => options,
      ),
      removedPriceLines:
        chartMocks.candlestickSeries.removePriceLine.mock.calls.length,
    })

    expect(readSelectedState()).toEqual({
      periodLineData: [
        [],
        [
          { time: 1744243200, value: 100 },
          { time: 1744329600, value: 100 },
        ],
        [],
      ],
      priceLines: [
        {
          price: 100,
          color: '#ef4444',
          lineWidth: 2,
          lineStyle: 2,
          lineVisible: false,
          axisLabelVisible: true,
          title: '架空分類',
          axisLabelColor: '#ef4444',
          axisLabelTextColor: '#ffffff',
        },
        {
          price: 100,
          color: '#ef4444',
          lineWidth: 2,
          lineStyle: 2,
          lineVisible: true,
          axisLabelVisible: true,
          title: '架空分類',
          axisLabelColor: '#ef4444',
          axisLabelTextColor: '#ffffff',
        },
      ],
      removedPriceLines: 1,
    })
  })
})
