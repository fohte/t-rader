import { act, cleanup, render, screen } from '@testing-library/react'
import type {
  CreatePriceLineOptions,
  MouseEventParams,
  Point,
  SeriesMarker,
  Time,
  UTCTimestamp,
} from 'lightweight-charts'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CandlestickChart } from '#components/candlestick-chart'
import type { ChartAnnotation } from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'
import { toChartTimestamp } from '#lib/chart-utils'

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
    coordinateToPrice: vi.fn<(coordinate: number) => number | null>(() => 100),
    priceScale: vi.fn(() => candlePriceScale),
    priceToCoordinate: vi.fn<(price: number) => number | null>(() => 30),
    removePriceLine: vi.fn(),
    setData: vi.fn(),
  }
  const annotationPriceSeries = {
    setData: vi.fn<(data: { time: UTCTimestamp; value: number }[]) => void>(),
  }
  const trendLinePreviewSeries = {
    setData: vi.fn<(data: { time: UTCTimestamp; value: number }[]) => void>(),
  }
  const indicatorSeries = {
    createPriceLine: vi.fn<(options: CreatePriceLineOptions) => object>(
      () => ({}),
    ),
    setData: vi.fn<(data: { time: UTCTimestamp; value: number }[]) => void>(),
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
    subscribeClick:
      vi.fn<(handler: (param: MouseEventParams) => void) => void>(),
    subscribeCrosshairMove:
      vi.fn<(handler: (param: MouseEventParams) => void) => void>(),
    timeScale: vi.fn(() => timeScale),
    unsubscribeClick:
      vi.fn<(handler: (param: MouseEventParams) => void) => void>(),
    unsubscribeCrosshairMove:
      vi.fn<(handler: (param: MouseEventParams) => void) => void>(),
  }
  chart.addSeries.mockImplementation((_series, _options, paneIndex = 0) => {
    while (paneApis.length <= paneIndex) paneApis.push(createPane())
    const index = chart.addSeries.mock.calls.length
    if (index === 1) return candlestickSeries
    if (index === 2) return annotationPriceSeries
    if (index === 3) return volumeSeries
    if (index === 4) return trendLinePreviewSeries
    return indicatorSeries
  })

  return {
    chart,
    candlePriceScale,
    candlestickSeries,
    annotationPriceSeries,
    trendLinePreviewSeries,
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

const secondBar: components['schemas']['Bar'] = {
  ...firstBar,
  close: 104,
  timestamp: '2025-04-11T00:00:00.000Z',
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
  created_by_kind: 'llm',
  price: 100,
  price_start: null,
  status: 'unread',
  target_kind: '架空分類',
  text: '架空の注釈',
  timestamp: '2025-04-10T00:00:00.000Z',
  timestamp_start: null,
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

function createChartPoint(x: number, y: number): Point {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- Lightweight Charts は座標をブランド型で受け取る
  return { x, y } as Point
}

beforeEach(() => {
  vi.clearAllMocks()
  chartMocks.candlestickSeries.coordinateToPrice.mockReturnValue(100)
  chartMocks.candlestickSeries.priceToCoordinate.mockReturnValue(30)
  chartMocks.timeScale.timeToCoordinate.mockReturnValue(36)
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
        .slice(4)
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

  it('places a horizontal line at the clicked bar and price', () => {
    const onPlaceHorizontalLine = vi.fn()
    chartMocks.candlestickSeries.coordinateToPrice.mockReturnValue(123.5)

    render(
      <CandlestickChart
        bars={bars}
        horizontalLineDrawingEnabled
        onPlaceHorizontalLine={onPlaceHorizontalLine}
      />,
    )
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]
    clickHandler?.({
      point: createChartPoint(36, 18),
      time: toChartTimestamp(firstBar.timestamp),
      seriesData: new Map(),
    })

    expect(onPlaceHorizontalLine.mock.calls).toEqual([
      [{ timestamp: firstBar.timestamp, price: 123.5 }],
    ])
  })

  it('places both trendline points with two chart clicks', () => {
    const onPlaceTrendLinePoint = vi.fn()
    chartMocks.candlestickSeries.coordinateToPrice
      .mockReturnValueOnce(3390)
      .mockReturnValueOnce(3120)

    render(
      <CandlestickChart
        bars={[firstBar, secondBar]}
        trendLineDrawingEnabled
        onPlaceTrendLinePoint={onPlaceTrendLinePoint}
      />,
    )
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]

    act(() => {
      clickHandler?.({
        point: createChartPoint(36, 18),
        time: toChartTimestamp(firstBar.timestamp),
        seriesData: new Map(),
      })
      clickHandler?.({
        point: createChartPoint(80, 30),
        time: toChartTimestamp(secondBar.timestamp),
        seriesData: new Map(),
      })
    })

    expect(onPlaceTrendLinePoint.mock.calls).toEqual([
      [{ timestamp: firstBar.timestamp, price: 3390 }],
      [{ timestamp: secondBar.timestamp, price: 3120 }],
    ])
  })

  it('tracks the cursor after the first trendline point and places the second point', () => {
    const onPlaceTrendLinePoint = vi.fn()
    const start = { timestamp: firstBar.timestamp, price: 3390 }
    chartMocks.candlestickSeries.coordinateToPrice.mockReturnValue(3120)

    render(
      <CandlestickChart
        bars={[firstBar, secondBar]}
        trendLineDrawingEnabled
        pendingTrendLineStart={start}
        onPlaceTrendLinePoint={onPlaceTrendLinePoint}
      />,
    )
    const crosshairHandler =
      chartMocks.chart.subscribeCrosshairMove.mock.calls.at(-1)?.[0]
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]

    act(() => {
      crosshairHandler?.({
        point: createChartPoint(80, 18),
        time: toChartTimestamp(secondBar.timestamp),
        seriesData: new Map(),
      })
      clickHandler?.({
        point: createChartPoint(80, 18),
        time: toChartTimestamp(secondBar.timestamp),
        seriesData: new Map(),
      })
    })

    function readTrendLineDrawingState() {
      return {
        previewData:
          chartMocks.trendLinePreviewSeries.setData.mock.calls.at(-1)?.[0],
        placedPoints: onPlaceTrendLinePoint.mock.calls,
      }
    }

    expect(readTrendLineDrawingState()).toEqual({
      previewData: [
        { time: toChartTimestamp(firstBar.timestamp), value: 3390 },
        { time: toChartTimestamp(secondBar.timestamp), value: 3120 },
      ],
      placedPoints: [[{ timestamp: secondBar.timestamp, price: 3120 }]],
    })
  })

  it('draws human horizontal annotations even when none is selected', () => {
    render(
      <CandlestickChart
        bars={bars}
        annotations={[{ ...pointAnnotation, created_by_kind: 'human' }]}
        selectedAnnotationId={null}
      />,
    )

    function readRenderedState() {
      return {
        priceLines: chartMocks.candlestickSeries.createPriceLine.mock.calls,
        annotationBand: screen.queryByRole('group', {
          name: 'チャートのアノテーション',
        }),
        volumeScaleOptions: chartMocks.volumePriceScale.applyOptions.mock.calls,
      }
    }

    expect(readRenderedState()).toEqual({
      priceLines: [
        [
          {
            price: 100,
            color: '#ffffff',
            lineWidth: 2,
            lineStyle: 0,
            lineVisible: true,
            axisLabelVisible: true,
            title: '架空分類',
            axisLabelColor: '#ffffff',
            axisLabelTextColor: '#111827',
          },
        ],
      ],
      annotationBand: null,
      volumeScaleOptions: [[{ scaleMargins: { top: 0.8, bottom: 0 } }]],
    })
  })

  it('draws and selects a saved human trendline without a band marker', () => {
    const trendLine: ChartAnnotation = {
      id: 'annotation-trendline',
      created_by_kind: 'human',
      target_kind: 'sample-kind',
      timestamp_start: firstBar.timestamp,
      price_start: 3390,
      timestamp: secondBar.timestamp,
      price: 3120,
      status: 'approved',
      text: '架空のトレンドラインです。',
    }
    const onSelectAnnotation = vi.fn()
    const startTime = toChartTimestamp(firstBar.timestamp)
    const endTime = toChartTimestamp(secondBar.timestamp)
    chartMocks.timeScale.timeToCoordinate.mockImplementation((time) =>
      time === startTime ? 20 : 80,
    )
    chartMocks.candlestickSeries.priceToCoordinate.mockImplementation(
      (price) => (price === 3390 ? 60 : 20),
    )

    render(
      <CandlestickChart
        bars={[firstBar, secondBar]}
        annotations={[trendLine]}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]

    act(() => {
      clickHandler?.({
        point: createChartPoint(50, 40),
        time: endTime,
        seriesData: new Map(),
      })
    })

    function readRenderedTrendLineState() {
      return {
        lineData: chartMocks.indicatorSeries.setData.mock.calls.map(
          ([data]) => data,
        ),
        annotationBand: screen.queryByRole('group', {
          name: 'チャートのアノテーション',
        }),
        selectedAnnotationIds: onSelectAnnotation.mock.calls,
      }
    }

    const renderedTrendLineState = readRenderedTrendLineState()

    expect(renderedTrendLineState).toEqual({
      lineData: [
        [
          { time: startTime, value: 3390 },
          { time: endTime, value: 3120 },
        ],
      ],
      annotationBand: null,
      selectedAnnotationIds: [['annotation-trendline']],
    })
  })

  it('clips and selects the visible part of a saved human trendline', () => {
    const trendLine: ChartAnnotation = {
      id: 'annotation-clipped-trendline',
      created_by_kind: 'human',
      target_kind: 'sample-kind',
      timestamp_start: '2025-04-09T00:00:00.000Z',
      price_start: 90,
      timestamp: '2025-04-12T00:00:00.000Z',
      price: 150,
      status: 'approved',
      text: '架空の傾きを記録する',
    }
    const onSelectAnnotation = vi.fn()
    const startTime = toChartTimestamp(firstBar.timestamp)
    const endTime = toChartTimestamp(secondBar.timestamp)
    chartMocks.timeScale.timeToCoordinate.mockImplementation((time) =>
      time === startTime ? 20 : 80,
    )
    chartMocks.candlestickSeries.priceToCoordinate.mockImplementation(
      (price) => (price === 110 ? 60 : 20),
    )

    render(
      <CandlestickChart
        bars={[firstBar, secondBar]}
        annotations={[trendLine]}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]

    act(() => {
      clickHandler?.({
        point: createChartPoint(50, 40),
        time: endTime,
        seriesData: new Map(),
      })
    })

    function readClippedTrendLineState() {
      return {
        lineData: chartMocks.indicatorSeries.setData.mock.calls.map(
          ([data]) => data,
        ),
        selectedAnnotationIds: onSelectAnnotation.mock.calls,
      }
    }

    expect(readClippedTrendLineState()).toEqual({
      lineData: [
        [
          { time: startTime, value: 110 },
          { time: endTime, value: 130 },
        ],
      ],
      selectedAnnotationIds: [['annotation-clipped-trendline']],
    })
  })

  it('shows a band marker when a weekly bar collapses a human trendline', () => {
    const weeklyBar = {
      ...firstBar,
      timeframe: '1w' as const,
      timestamp: '2030-04-01T00:00:00Z',
    }
    const trendLine: ChartAnnotation = {
      id: 'annotation-weekly-trendline',
      created_by_kind: 'human',
      target_kind: 'sample-kind',
      timestamp_start: '2030-04-02T06:00:00Z',
      price_start: 3390,
      timestamp: '2030-04-03T06:00:00Z',
      price: 3120,
      status: 'approved',
      text: '架空の傾きを記録する',
    }

    render(<CandlestickChart bars={[weeklyBar]} annotations={[trendLine]} />)

    function readCollapsedTrendLineState() {
      return {
        annotationBand: screen
          .queryByRole('group', { name: 'チャートのアノテーション' })
          ?.getAttribute('aria-label'),
        markerButton: screen
          .queryByRole('button', { name: 'アノテーションを選択' })
          ?.getAttribute('aria-label'),
        lineCalls: chartMocks.indicatorSeries.setData.mock.calls,
      }
    }

    expect(readCollapsedTrendLineState()).toEqual({
      annotationBand: 'チャートのアノテーション',
      markerButton: 'アノテーションを選択',
      lineCalls: [],
    })
  })

  it('lets chart removal dispose of horizontal price lines on unmount', () => {
    const { unmount } = render(
      <CandlestickChart
        bars={bars}
        annotations={[{ ...pointAnnotation, created_by_kind: 'human' }]}
      />,
    )

    unmount()

    function readRemovalState() {
      return {
        chartRemovalCalls: chartMocks.chart.remove.mock.calls,
        priceLineRemovalCalls:
          chartMocks.candlestickSeries.removePriceLine.mock.calls,
      }
    }

    expect(readRemovalState()).toEqual({
      chartRemovalCalls: [[]],
      priceLineRemovalCalls: [],
    })
  })

  it('selects a human horizontal annotation when its chart line is clicked', () => {
    const onSelectAnnotation = vi.fn()
    const humanLine = { ...pointAnnotation, created_by_kind: 'human' }

    render(
      <CandlestickChart
        bars={bars}
        annotations={[humanLine]}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )
    const clickHandler = chartMocks.chart.subscribeClick.mock.calls.at(-1)?.[0]
    clickHandler?.({
      point: createChartPoint(36, 35),
      time: toChartTimestamp(firstBar.timestamp),
      seriesData: new Map(),
    })

    expect(onSelectAnnotation.mock.calls).toEqual([['annotation-a']])
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
