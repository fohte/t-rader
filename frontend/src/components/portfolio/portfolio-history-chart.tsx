import { SegmentedControl } from '@fohte/ui/segmented-control'
import {
  AreaSeries,
  createChart,
  type IChartApi,
  type ISeriesApi,
  LineSeries,
  LineStyle,
  type Time,
} from 'lightweight-charts'
import type { CSSProperties } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'

import { formatYen, pnlColorClass } from '#components/trades/format'
import { Skeleton } from '#components/ui/skeleton'
import {
  getChartThemeColors,
  getChartThemeOptions,
  observeChartResize,
  observeChartTheme,
} from '#lib/chart-theme'
import {
  addPortfolioHistoryChanges,
  type PortfolioHistoryPeriod,
  type PortfolioHistoryPoint,
  selectPortfolioHistoryPoints,
} from '#lib/portfolio-history'

type HistoryStatus = 'loading' | 'empty' | 'error' | 'ready'
type LegendColorStyle = CSSProperties & { '--legend-color': string }

interface PortfolioHistoryChartProps {
  data: PortfolioHistoryPoint[]
  period: PortfolioHistoryPeriod
  status: HistoryStatus
  errorMessage?: string
  initialHoveredDate?: string
  onPeriodChange: (period: PortfolioHistoryPeriod) => void
}

const periods: { value: PortfolioHistoryPeriod; label: string }[] = [
  { value: 'day', label: '日' },
  { value: 'week', label: '週' },
  { value: 'month', label: '月' },
  { value: 'year', label: '年' },
]

const legendColors = {
  stock: 'var(--chart-1)',
  cash: 'var(--chart-2)',
  total: 'var(--foreground)',
  investable: 'var(--chart-3)',
}

function oklchToRgb(color: string): string | null {
  const match =
    /^oklch\(\s*(\d*\.?\d+)\s+(\d*\.?\d+)\s+(-?\d*\.?\d+)(?:deg)?\s*\)$/i.exec(
      color,
    )
  if (match == null) return null

  const lightness = Number(match[1])
  const chroma = Number(match[2])
  const hue = (Number(match[3]) * Math.PI) / 180
  if (![lightness, chroma, hue].every(Number.isFinite)) return null

  const a = chroma * Math.cos(hue)
  const b = chroma * Math.sin(hue)
  const lightnessL = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const lightnessM = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const lightnessS = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3

  const linearRgb = [
    4.0767416621 * lightnessL -
      3.3077115913 * lightnessM +
      0.2309699292 * lightnessS,
    -1.2684380046 * lightnessL +
      2.6097574011 * lightnessM -
      0.3413193965 * lightnessS,
    -0.0041960863 * lightnessL -
      0.7034186147 * lightnessM +
      1.707614701 * lightnessS,
  ]
  const srgb = linearRgb.map((channel) => {
    const clipped = Math.max(0, Math.min(1, channel))
    const encoded =
      clipped <= 0.0031308
        ? 12.92 * clipped
        : 1.055 * clipped ** (1 / 2.4) - 0.055
    return Math.round(encoded * 255)
  })

  return `rgb(${srgb.join(', ')})`
}

function readThemeColor(name: string): string {
  // Lightweight Charts の color parser は OKLCH を扱えないため sRGB に変換する。
  const color = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim()
  return oklchToRgb(color) ?? color
}

function getPortfolioSeriesColors() {
  const stock = readThemeColor('--chart-1')
  const cash = readThemeColor('--chart-2')

  return {
    stock,
    stockFillTop: `color-mix(in srgb, ${stock} 50%, transparent)`,
    stockFillBottom: `color-mix(in srgb, ${stock} 24%, transparent)`,
    cash,
    cashFillTop: `color-mix(in srgb, ${cash} 32%, transparent)`,
    cashFillBottom: `color-mix(in srgb, ${cash} 8%, transparent)`,
    total: readThemeColor('--foreground'),
    investable: readThemeColor('--chart-3'),
  }
}

function timeToDateKey(time: Time | undefined): string | undefined {
  if (typeof time === 'string') return time
  if (typeof time === 'number') {
    return new Date(time * 1000).toISOString().slice(0, 10)
  }
  if (time == null) return undefined

  const month = String(time.month).padStart(2, '0')
  const day = String(time.day).padStart(2, '0')
  const year = String(time.year).padStart(4, '0')
  return `${year}-${month}-${day}`
}

function LegendItem({
  label,
  color,
  kind,
}: {
  label: string
  color: string
  kind: 'line' | 'area' | 'dashed'
}) {
  const style: LegendColorStyle = { '--legend-color': color }

  return (
    <span className="inline-flex items-center gap-1.5 font-mono text-2xs text-muted-foreground-strong">
      {kind === 'area' ? (
        <span className="size-2.5 bg-(--legend-color)" style={style} />
      ) : (
        <span
          aria-hidden="true"
          className={
            kind === 'dashed'
              ? 'w-3 border-t border-dashed border-(--legend-color)'
              : 'w-3 border-t-2 border-(--legend-color)'
          }
          style={style}
        />
      )}
      {label}
    </span>
  )
}

export function PortfolioHistoryChart({
  data,
  period,
  status,
  errorMessage,
  initialHoveredDate,
  onPeriodChange,
}: PortfolioHistoryChartProps) {
  const points = useMemo(
    () => selectPortfolioHistoryPoints(data, period),
    [data, period],
  )
  const isReady = status === 'ready' && points.length > 0

  return (
    <section className="border border-border bg-card">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          <span className="mr-2 text-muted-foreground">&gt;</span>
          資産推移
        </h2>
        <SegmentedControl
          aria-label="表示単位"
          options={periods}
          value={period}
          onValueChange={onPeriodChange}
        />
      </header>

      <div className="px-3.5 py-3">
        {status === 'loading' && <Skeleton className="h-64 w-full" />}
        {status === 'error' && (
          <div className="flex h-64 items-center justify-center border border-dashed border-border px-4 text-center text-sm text-muted-foreground-strong">
            {errorMessage ?? '資産推移を読み込めませんでした'}
          </div>
        )}
        {(status === 'empty' ||
          (status === 'ready' && points.length === 0)) && (
          <div className="flex h-64 items-center justify-center border border-dashed border-border px-4 text-center text-sm text-muted-foreground-strong">
            取引がありません。資産推移は取引の登録後に表示されます。
          </div>
        )}
        {isReady && (
          <PortfolioHistoryPlot
            points={points}
            period={period}
            initialHoveredDate={initialHoveredDate}
          />
        )}

        {isReady && (
          <div
            aria-label="凡例"
            className="mt-2 flex flex-wrap gap-x-4 gap-y-2"
          >
            <LegendItem label="総資産" color={legendColors.total} kind="line" />
            <LegendItem
              label="株式 (時価)"
              color={legendColors.stock}
              kind="area"
            />
            <LegendItem label="現金" color={legendColors.cash} kind="area" />
            <LegendItem
              label="元手"
              color={legendColors.investable}
              kind="dashed"
            />
          </div>
        )}

        {isReady && (
          <p className="mt-1.5 font-mono text-2xs text-muted-foreground">
            資産推移の最終日: {data.at(-1)?.date}
          </p>
        )}
      </div>
    </section>
  )
}

function PortfolioHistoryPlot({
  points,
  period,
  initialHoveredDate,
}: {
  points: PortfolioHistoryPoint[]
  period: PortfolioHistoryPeriod
  initialHoveredDate?: string
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const totalSeriesRef = useRef<ISeriesApi<'Area'> | null>(null)
  const stockSeriesRef = useRef<ISeriesApi<'Area'> | null>(null)
  const investableSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const pointsByDate = useMemo(
    () =>
      new Map(
        addPortfolioHistoryChanges(points).map((item) => [
          item.point.date,
          item,
        ]),
      ),
    [points],
  )
  const [hoveredDate, setHoveredDate] = useState<string | null>(
    initialHoveredDate ?? null,
  )
  const [hoveredPeriod, setHoveredPeriod] = useState(period)
  if (hoveredPeriod !== period) {
    setHoveredPeriod(period)
    setHoveredDate(null)
  }
  const hoveredPoint =
    hoveredDate == null ? null : (pointsByDate.get(hoveredDate) ?? null)

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const themeColors = getChartThemeColors(
      document.documentElement.classList.contains('dark'),
    )
    const chart = createChart(container, {
      width: container.clientWidth,
      height: 256,
      ...getChartThemeOptions(themeColors),
      localization: { locale: 'ja-JP' },
    })
    chartRef.current = chart

    const compactYen = new Intl.NumberFormat('ja-JP', {
      style: 'currency',
      currency: 'JPY',
      notation: 'compact',
      maximumFractionDigits: 1,
    })
    const priceFormat = {
      type: 'custom' as const,
      minMove: 1,
      formatter: (price: number) => compactYen.format(price),
    }

    const seriesColors = getPortfolioSeriesColors()
    const totalSeries = chart.addSeries(AreaSeries, {
      lineColor: seriesColors.total,
      topColor: seriesColors.cashFillTop,
      bottomColor: seriesColors.cashFillBottom,
      lineWidth: 2,
      priceFormat,
      priceLineVisible: false,
      lastValueVisible: false,
    })
    const stockSeries = chart.addSeries(AreaSeries, {
      lineColor: seriesColors.stock,
      topColor: seriesColors.stockFillTop,
      bottomColor: seriesColors.stockFillBottom,
      lineWidth: 1,
      priceFormat,
      priceLineVisible: false,
      lastValueVisible: false,
    })
    const investableSeries = chart.addSeries(LineSeries, {
      color: seriesColors.investable,
      lineWidth: 1,
      lineStyle: LineStyle.Dashed,
      priceFormat,
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
    })
    totalSeriesRef.current = totalSeries
    stockSeriesRef.current = stockSeries
    investableSeriesRef.current = investableSeries

    const disconnectResizeObserver = observeChartResize(
      container,
      (width, height) => {
        chart.applyOptions({ width, height })
      },
    )

    const disconnectThemeObserver = observeChartTheme(() => {
      const colors = getChartThemeColors(
        document.documentElement.classList.contains('dark'),
      )
      chart.applyOptions(getChartThemeOptions(colors))
      const seriesColors = getPortfolioSeriesColors()
      totalSeries.applyOptions({
        lineColor: seriesColors.total,
        topColor: seriesColors.cashFillTop,
        bottomColor: seriesColors.cashFillBottom,
      })
      stockSeries.applyOptions({
        lineColor: seriesColors.stock,
        topColor: seriesColors.stockFillTop,
        bottomColor: seriesColors.stockFillBottom,
      })
      investableSeries.applyOptions({ color: seriesColors.investable })
    })

    const handleCrosshairMove = (param: { time?: Time }) => {
      const date = timeToDateKey(param.time)
      setHoveredDate(date ?? null)
    }
    chart.subscribeCrosshairMove(handleCrosshairMove)

    return () => {
      chart.unsubscribeCrosshairMove(handleCrosshairMove)
      disconnectThemeObserver()
      disconnectResizeObserver()
      chart.remove()
      chartRef.current = null
      totalSeriesRef.current = null
      stockSeriesRef.current = null
      investableSeriesRef.current = null
    }
  }, [])

  useEffect(() => {
    const totalSeries = totalSeriesRef.current
    const stockSeries = stockSeriesRef.current
    const investableSeries = investableSeriesRef.current
    if (!totalSeries || !stockSeries || !investableSeries) return

    totalSeries.setData(
      points.map((point) => ({ time: point.date, value: point.total_assets })),
    )
    stockSeries.setData(
      points.map((point) => ({
        time: point.date,
        value: point.stock_market_value,
      })),
    )
    investableSeries.setData(
      points.map((point) => ({
        time: point.date,
        value: point.investable_amount,
      })),
    )

    const maxValue = Math.max(
      1,
      ...points.map((point) =>
        Math.max(point.total_assets, point.investable_amount),
      ),
    )
    totalSeries.applyOptions({
      autoscaleInfoProvider: () => ({
        priceRange: { minValue: 0, maxValue: maxValue * 1.04 },
      }),
    })
    chartRef.current?.timeScale().fitContent()
  }, [points])

  return (
    <div className="relative">
      <div ref={containerRef} className="h-64 w-full" />
      {hoveredPoint != null && (
        <div
          role="tooltip"
          className="pointer-events-none absolute left-2 top-2 z-10 min-w-44 border border-border bg-card/95 px-3 py-2 shadow-sm"
        >
          <p className="mb-1 font-mono text-2xs text-muted-foreground">
            {hoveredPoint.point.date}
          </p>
          <dl className="space-y-0.5 text-xs">
            <div className="flex justify-between gap-3">
              <dt>総資産</dt>
              <dd className="shrink-0 text-right font-mono">
                {formatYen(hoveredPoint.point.total_assets)}
              </dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt>株式 (時価)</dt>
              <dd className="shrink-0 text-right font-mono">
                {formatYen(hoveredPoint.point.stock_market_value)}
              </dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt>現金</dt>
              <dd className="shrink-0 text-right font-mono">
                {formatYen(hoveredPoint.point.cash)}
              </dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt>前期比</dt>
              <dd
                className={`shrink-0 text-right font-mono ${
                  hoveredPoint.change == null
                    ? 'text-muted-foreground-strong'
                    : pnlColorClass(hoveredPoint.change)
                }`}
              >
                {hoveredPoint.change == null
                  ? '—'
                  : formatYen(hoveredPoint.change, true)}
              </dd>
            </div>
          </dl>
        </div>
      )}
    </div>
  )
}
