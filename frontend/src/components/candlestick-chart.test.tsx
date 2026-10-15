import { act, cleanup, render, screen } from '@testing-library/react'
import type {
  CreatePriceLineOptions,
  SeriesMarker,
  Time,
  UTCTimestamp,
} from 'lightweight-charts'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CandlestickChart } from '#components/candlestick-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

const chartMocks = vi.hoisted(() => {
  const candlePriceScale = { applyOptions: vi.fn() }
  const paneApis: {
    getHeight: ReturnType<typeof vi.fn>
    setStretchFactor: ReturnType<typeof vi.fn>
  }[] = []
  const createPane = (initialHeight = 220) => {
    let height = initialHeight
    return {
      getHeight: vi.fn(() => height),
      setStretchFactor: vi.fn((stretchFactor: number) => {
        height = stretchFactor
      }),
    }
  }
  const resetPanes = () => {
    paneApis.splice(0, paneApis.length, createPane())
  }
  resetPanes()
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
  const indicatorSeries = {
    createPriceLine: vi.fn<(options: CreatePriceLineOptions) => object>(
      () => ({}),
    ),
    setData: vi.fn(),
  }
  const seriesMarkers = {
    detach: vi.fn(),
    markers: vi.fn(),
    setMarkers: vi.fn<(markers: SeriesMarker<Time>[]) => void>(),
  }
  const volumeSeries = {
    priceScale: vi.fn(() => volumePriceScale),
    setData: vi.fn(),
  }
  const timeScale = {
    fitContent: vi.fn(),
    getVisibleRange: vi.fn<
      () => {
        from: string
        to: string
      } | null
    >(() => null),
    height: vi.fn(() => 24),
    setVisibleLogicalRange: vi.fn(),
    setVisibleRange: vi.fn(),
    subscribeSizeChange: vi.fn(),
    subscribeVisibleTimeRangeChange: vi.fn(),
    timeToCoordinate: vi.fn<(time: UTCTimestamp) => number>(() => 36),
    unsubscribeSizeChange: vi.fn(),
    unsubscribeVisibleTimeRangeChange: vi.fn(),
    width: vi.fn(() => 120),
  }
  const chart = {
    addSeries: vi.fn<
      (series: unknown, options?: unknown, paneIndex?: number) => unknown
    >(() => null),
    applyOptions: vi.fn(),
    panes: vi.fn(() => paneApis),
    removePane: vi.fn((index: number) => {
      paneApis.splice(index, 1)
    }),
    removeSeries: vi.fn(),
    remove: vi.fn(),
    timeScale: vi.fn(() => timeScale),
  }
  chart.addSeries.mockImplementation((_series, _options, paneIndex = 0) => {
    while (paneApis.length <= paneIndex) paneApis.push(createPane())
    const index = chart.addSeries.mock.calls.length
    if (index === 1) return candlestickSeries
    if (index === 2) return annotationPriceSeries
    if (index === 3) return volumeSeries
    return indicatorSeries
  })

  return {
    chart,
    candlePriceScale,
    candlestickSeries,
    annotationPriceSeries,
    indicatorSeries,
    volumePriceScale,
    paneApis,
    resetPanes,
    timeScale,
    seriesMarkers,
  }
})

vi.mock('lightweight-charts', () => ({
  CandlestickSeries: {},
  ColorType: { Solid: 'solid' },
  createChart: vi.fn(() => chartMocks.chart),
  createSeriesMarkers: vi.fn(() => chartMocks.seriesMarkers),
  HistogramSeries: {},
  LineSeries: {},
  LineStyle: { Dashed: 2, Solid: 0 },
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
type Trade = components['schemas']['TradeListItem']

function makeTrade(overrides: Partial<Trade> = {}): Trade {
  return {
    id: '00000000-0000-0000-0000-000000000101',
    strategy_id: '00000000-0000-0000-0000-000000000001',
    symbol: 'demo-code',
    side: 'buy',
    qty: 1,
    price: 100,
    fee: 0,
    date: '2025-04-10',
    source: 'manual',
    note: null,
    note_count: 0,
    created_at: '2025-04-10T00:00:00.000Z',
    updated_at: '2025-04-10T00:00:00.000Z',
    ...overrides,
  }
}

const pointAnnotation: ChartAnnotation = {
  id: 'annotation-a',
  price: 100,
  status: 'unread',
  target_kind: '架空分類',
  text: '架空の注釈',
  timestamp: '2025-04-10T00:00:00.000Z',
}

const annotations: ChartAnnotation[] = [pointAnnotation]
const pendingAnimationFrames = new Map<number, FrameRequestCallback>()
const resizeObserverCallbacks: ResizeObserverCallback[] = []
const resizeObservers: ResizeObserver[] = []
let nextAnimationFrameId = 0

function flushAnimationFrames(): void {
  act(() => {
    const callbacks = [...pendingAnimationFrames.values()]
    pendingAnimationFrames.clear()
    for (const callback of callbacks) callback(0)
  })
}

function resizeChart(height: number): void {
  act(() => {
    const observer = resizeObservers[0]
    if (observer == null) return

    const entry: ResizeObserverEntry = {
      borderBoxSize: [],
      contentBoxSize: [],
      contentRect: new DOMRectReadOnly(0, 0, 720, height),
      devicePixelContentBoxSize: [],
      target: document.createElement('div'),
    }
    for (const callback of resizeObserverCallbacks) {
      callback([entry], observer)
    }
  })
}

class TestResizeObserver implements ResizeObserver {
  constructor(callback: ResizeObserverCallback) {
    resizeObserverCallbacks.push(callback)
    resizeObservers.push(this)
  }

  observe(target: Element): void {
    void target
  }

  unobserve(target: Element): void {
    void target
  }

  disconnect(): void {
    resizeObserverCallbacks.splice(0)
    resizeObservers.splice(0)
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  chartMocks.resetPanes()
  pendingAnimationFrames.clear()
  resizeObserverCallbacks.length = 0
  resizeObservers.length = 0
  nextAnimationFrameId = 0
  vi.stubGlobal('ResizeObserver', TestResizeObserver)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    nextAnimationFrameId += 1
    pendingAnimationFrames.set(nextAnimationFrameId, callback)
    return nextAnimationFrameId
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    pendingAnimationFrames.delete(id)
  })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('CandlestickChart', () => {
  it('draws a price overlay from lookback bars without widening the initial view', () => {
    const indicatorBars = [
      { ...firstBar, timestamp: '2025-04-10T00:00:00.000Z', close: 10 },
      { ...firstBar, timestamp: '2025-04-11T00:00:00.000Z', close: 20 },
      { ...firstBar, timestamp: '2025-04-12T00:00:00.000Z', close: 30 },
      { ...firstBar, timestamp: '2025-04-13T00:00:00.000Z', close: 40 },
    ]

    render(
      <CandlestickChart
        bars={indicatorBars}
        indicators={[
          { id: 'indicator-sma', kind: 'sma', period: 2, color: '#f97316' },
        ]}
        displayRangeStart="2025-04-12"
      />,
    )

    const readRenderedState = () => ({
      lineData: chartMocks.indicatorSeries.setData.mock.calls,
      visibleRanges: chartMocks.timeScale.setVisibleLogicalRange.mock.calls,
      legend: screen.getByRole('list', { name: '表示中のチャート指標' })
        .textContent,
    })

    expect(readRenderedState()).toEqual({
      lineData: [
        [
          [
            { time: 1744329600, value: 15 },
            { time: 1744416000, value: 25 },
            { time: 1744502400, value: 35 },
          ],
        ],
      ],
      visibleRanges: [[{ from: 1.5, to: 3.5 }]],
      legend: 'SMA 2',
    })
  })

  it('adds eighty-pixel lower panes without shrinking the price pane', () => {
    render(
      <CandlestickChart
        bars={bars}
        indicators={[
          { id: 'indicator-rsi', kind: 'rsi', period: 14, color: '#38bdf8' },
          {
            id: 'indicator-macd',
            kind: 'macd',
            fastPeriod: 12,
            slowPeriod: 26,
            signalPeriod: 9,
            color: '#34d399',
          },
        ]}
      />,
    )
    resizeChart(380)
    flushAnimationFrames()

    const paneState = () => ({
      seriesPaneIndexes: chartMocks.chart.addSeries.mock.calls
        .slice(3)
        .map(([, , paneIndex]) => paneIndex ?? 0),
      paneStretchFactors: chartMocks.paneApis.map(
        (pane) => pane.setStretchFactor.mock.lastCall,
      ),
      lowerPaneLegends: [
        screen.getByRole('list', { name: '下段 1 の表示中の指標' }).textContent,
        screen.getByRole('list', { name: '下段 2 の表示中の指標' }).textContent,
      ],
      lowerPaneLegendTops: [
        screen.getByRole('list', { name: '下段 1 の表示中の指標' }).style.top,
        screen.getByRole('list', { name: '下段 2 の表示中の指標' }).style.top,
      ],
      resizeHeight: chartMocks.chart.applyOptions.mock.calls.at(-1),
    })

    expect(paneState()).toEqual({
      seriesPaneIndexes: [1, 2, 2, 2],
      paneStretchFactors: [[196], [80], [80]],
      lowerPaneLegends: ['RSI 14', 'MACD 12 26 9'],
      lowerPaneLegendTops: ['204px', '284px'],
      resizeHeight: [{ width: 720, height: 380 }],
    })
  })

  it('keeps the visible time range when extended bars arrive', () => {
    const viewport = {
      from: '2025-04-10',
      to: '2025-04-10',
    }
    const extendedBars = [
      { ...firstBar, timestamp: '2025-04-09T00:00:00.000Z' },
      firstBar,
    ]
    const { rerender } = render(
      <CandlestickChart bars={[firstBar]} displayRangeStart="2025-04-10" />,
    )

    chartMocks.timeScale.getVisibleRange.mockReturnValue(viewport)
    rerender(
      <CandlestickChart bars={extendedBars} displayRangeStart="2025-04-10" />,
    )

    const readUpdatedViewport = () => ({
      restoredRanges: chartMocks.timeScale.setVisibleRange.mock.calls,
      capturedBeforeDataUpdate: (() => {
        const rangeReadOrder =
          chartMocks.timeScale.getVisibleRange.mock.invocationCallOrder[1]
        const dataUpdateOrder =
          chartMocks.candlestickSeries.setData.mock.invocationCallOrder[1]
        return (
          rangeReadOrder != null &&
          dataUpdateOrder != null &&
          rangeReadOrder < dataUpdateOrder
        )
      })(),
    })

    expect(readUpdatedViewport()).toEqual({
      restoredRanges: [[viewport]],
      capturedBeforeDataUpdate: true,
    })
  })

  it('places a marker at the selected daily bar', () => {
    render(
      <CandlestickChart
        bars={bars}
        highlightedTimestamp={firstBar.timestamp}
      />,
    )

    const readRenderedState = () => ({
      markerStyle: screen
        .getByRole('img', { name: '2025-04-10 のチャート上の位置' })
        .getAttribute('style'),
      targetTimes: chartMocks.timeScale.timeToCoordinate.mock.calls.map(
        ([time]) => time,
      ),
    })

    expect(readRenderedState()).toEqual({
      markerStyle: 'left: 36px; bottom: 24px;',
      targetTimes: [1744243200],
    })
  })

  it('updates trade markers when trades arrive after chart mount', () => {
    const { rerender } = render(<CandlestickChart bars={bars} trades={[]} />)

    rerender(<CandlestickChart bars={bars} trades={[makeTrade()]} />)

    expect(chartMocks.seriesMarkers.setMarkers.mock.calls).toEqual([
      [[]],
      [
        [
          {
            id: '00000000-0000-0000-0000-000000000101',
            time: 1744243200,
            position: 'belowBar',
            color: '#26a69a',
            shape: 'arrowUp',
          },
        ],
      ],
    ])
  })

  it('labels the selected trade marker after same-day markers', () => {
    const otherTrade = makeTrade({ id: 'fake-other-trade' })
    const selectedTrade = makeTrade({ id: 'fake-selected-trade' })

    render(
      <CandlestickChart
        bars={bars}
        trades={[selectedTrade, otherTrade]}
        selectedTradeId={selectedTrade.id}
      />,
    )

    expect(chartMocks.seriesMarkers.setMarkers.mock.calls).toEqual([
      [
        [
          {
            id: 'fake-other-trade',
            time: 1744243200,
            position: 'belowBar',
            color: '#26a69a',
            shape: 'arrowUp',
          },
          {
            id: 'fake-selected-trade',
            time: 1744243200,
            position: 'belowBar',
            color: '#26a69a',
            shape: 'arrowUp',
            text: '対象',
            size: 2,
          },
        ],
      ],
    ])
  })

  it('keeps trade markers empty when the optional prop is omitted', () => {
    render(<CandlestickChart bars={bars} />)

    expect(chartMocks.seriesMarkers.setMarkers.mock.calls).toEqual([[[]]])
  })

  it.each([
    ['default', undefined, 0.05],
    ['trade detail', 0.25, 0.25],
  ])('applies the %s top price scale margin', (_, topMargin, expectedTop) => {
    render(<CandlestickChart bars={bars} priceScaleTopMargin={topMargin} />)

    expect(chartMocks.candlePriceScale.applyOptions.mock.calls).toEqual([
      [{ scaleMargins: { top: expectedTop, bottom: 0.25 } }],
    ])
  })

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
