import { Button } from '@fohte/ui/button'
import {
  createChart,
  type IChartApi,
  type ISeriesApi,
  LineSeries,
} from 'lightweight-charts'
import { useEffect, useRef } from 'react'

import {
  getChartThemeColors,
  getChartThemeOptions,
  observeChartResize,
  observeChartTheme,
} from '#lib/chart-theme'
import {
  type IndicatorObservation,
  type IndicatorPeriod,
} from '#lib/indicator-observation'
import { cn } from '#lib/utils'

interface IndicatorObservationChartProps {
  observations: IndicatorObservation[]
  period: IndicatorPeriod
  onPeriodChange: (period: IndicatorPeriod) => void
  isPending: boolean
  isError: boolean
}

const PERIODS: { value: IndicatorPeriod; label: string }[] = [
  { value: '1m', label: '1M' },
  { value: '3m', label: '3M' },
  { value: '1y', label: '1Y' },
  { value: 'all', label: '全期間' },
]

export function IndicatorObservationChart({
  observations,
  period,
  onPeriodChange,
  isPending,
  isError,
}: IndicatorObservationChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const lineSeriesRef = useRef<ISeriesApi<'Line'> | null>(null)
  const shouldShowChart = !isPending && !isError && observations.length > 0

  useEffect(() => {
    const container = containerRef.current
    if (!shouldShowChart || container == null) return

    const isDark = document.documentElement.classList.contains('dark')
    const colors = getChartThemeColors(isDark)
    const chart = createChart(container, {
      ...getChartThemeOptions(colors),
      width: container.clientWidth,
      height: container.clientHeight,
    })
    chartRef.current = chart

    const lineSeries = chart.addSeries(LineSeries, {
      color: getLineColor(isDark),
      lineWidth: 2,
      priceLineVisible: false,
      crosshairMarkerVisible: true,
    })
    lineSeriesRef.current = lineSeries

    const disconnectResizeObserver = observeChartResize(
      container,
      (width, height) => {
        chart.applyOptions({ width, height })
      },
    )

    const disconnectThemeObserver = observeChartTheme(() => {
      const dark = document.documentElement.classList.contains('dark')
      const nextColors = getChartThemeColors(dark)
      chart.applyOptions(getChartThemeOptions(nextColors))
      lineSeries.applyOptions({ color: getLineColor(dark) })
    })

    return () => {
      disconnectThemeObserver()
      disconnectResizeObserver()
      chart.remove()
      chartRef.current = null
      lineSeriesRef.current = null
    }
  }, [shouldShowChart])

  useEffect(() => {
    const lineSeries = lineSeriesRef.current
    if (!shouldShowChart || lineSeries == null) return

    lineSeries.setData(
      observations.map((observation) => ({
        time: observation.date,
        value: observation.value,
      })),
    )
    chartRef.current?.timeScale().fitContent()
  }, [observations, shouldShowChart])

  const message = isPending
    ? '推移を読み込み中です。'
    : isError
      ? '推移を読み込めませんでした。'
      : 'この期間の観測データはありません。'

  return (
    <section
      className="space-y-3"
      aria-labelledby="indicator-observation-title"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2
          id="indicator-observation-title"
          className="font-mono text-xs font-bold uppercase tracking-wider text-foreground"
        >
          推移
        </h2>
        <div className="inline-flex gap-1" role="group" aria-label="表示期間">
          {PERIODS.map((option) => (
            <Button
              key={option.value}
              variant={period === option.value ? 'default' : 'outline'}
              size="xs"
              onClick={() => {
                onPeriodChange(option.value)
              }}
              aria-pressed={period === option.value}
            >
              {option.label}
            </Button>
          ))}
        </div>
      </div>
      {shouldShowChart ? (
        <div
          ref={containerRef}
          className="h-64 w-full border border-border bg-card"
        />
      ) : (
        <div className="flex h-64 items-center justify-center border border-border bg-card px-4">
          <p
            className={cn(
              'font-mono text-xs',
              isError ? 'text-primary' : 'text-muted-foreground',
            )}
            role={isPending ? 'status' : isError ? 'alert' : undefined}
          >
            {message}
          </p>
        </div>
      )}
    </section>
  )
}

function getLineColor(isDark: boolean): string {
  return isDark ? '#60a5fa' : '#2563eb'
}
