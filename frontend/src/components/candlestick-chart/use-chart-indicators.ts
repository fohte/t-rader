import {
  HistogramSeries,
  type IChartApi,
  type ISeriesApi,
  LineSeries,
  LineStyle,
  type SeriesType,
} from 'lightweight-charts'
import { useEffect } from 'react'

import type { components } from '#lib/api/schema.gen'
import { calculateChartIndicatorLines } from '#lib/chart-indicator-calculations'
import {
  type ChartIndicatorConfig,
  getChartIndicatorPaneCount,
  getChartIndicatorPaneIndexes,
} from '#lib/chart-indicator-settings'
import { toCandlestickData } from '#lib/chart-utils'

type Bar = components['schemas']['Bar']

interface ChartRef<T> {
  current: T | null
}

export function useChartIndicators({
  chartRef,
  indicatorSeriesRef,
  lowerPaneCountRef,
  updatePaneLayoutRef,
  bars,
  indicators,
}: {
  chartRef: ChartRef<IChartApi>
  indicatorSeriesRef: ChartRef<Map<string, ISeriesApi<SeriesType>[]>>
  lowerPaneCountRef: { current: number }
  updatePaneLayoutRef: { current: () => void }
  bars: Bar[]
  indicators: ChartIndicatorConfig[]
}) {
  useEffect(() => {
    const chart = chartRef.current
    const indicatorSeriesMap = indicatorSeriesRef.current
    if (chart == null || indicatorSeriesMap == null) return

    for (const series of indicatorSeriesMap.values()) {
      for (const indicatorSeries of series) {
        chart.removeSeries(indicatorSeries)
      }
    }
    indicatorSeriesMap.clear()
    while (chart.panes().length > 1) {
      chart.removePane(chart.panes().length - 1)
    }

    const paneIndexes = getChartIndicatorPaneIndexes(indicators)
    lowerPaneCountRef.current = getChartIndicatorPaneCount(indicators)

    const indicatorBars = toCandlestickData(bars).map(({ time, close }) => ({
      time,
      close,
    }))

    for (const [indicatorIndex, indicator] of indicators.entries()) {
      const lines = calculateChartIndicatorLines(indicator, indicatorBars)
      const paneIndex = paneIndexes[indicatorIndex] ?? 0

      const series: ISeriesApi<SeriesType>[] = []
      for (const { variant, points } of lines) {
        if (variant === 'histogram') {
          const histogramSeries = chart.addSeries(
            HistogramSeries,
            {
              priceScaleId: 'right',
              priceLineVisible: false,
              lastValueVisible: false,
            },
            paneIndex,
          )
          histogramSeries.setData(
            points.map((point) => ({
              ...point,
              color:
                point.value >= 0
                  ? 'rgba(38, 166, 154, 0.7)'
                  : 'rgba(239, 83, 80, 0.7)',
            })),
          )
          series.push(histogramSeries)
          continue
        }

        const lineSeries = chart.addSeries(
          LineSeries,
          {
            color: variant === 'signal' ? '#a1a1aa' : indicator.color,
            lineWidth: variant === 'band' || variant === 'signal' ? 1 : 2,
            lineStyle:
              variant === 'band' || variant === 'signal'
                ? LineStyle.Dashed
                : LineStyle.Solid,
            priceScaleId: 'right',
            priceLineVisible: false,
            lastValueVisible: false,
            crosshairMarkerVisible: false,
            pointMarkersVisible: false,
            ...(indicator.kind === 'rsi'
              ? {
                  autoscaleInfoProvider: () => ({
                    priceRange: { minValue: 0, maxValue: 100 },
                  }),
                }
              : {}),
          },
          paneIndex,
        )
        lineSeries.setData(points)

        if (indicator.kind === 'rsi') {
          for (const price of [30, 70]) {
            lineSeries.createPriceLine({
              price,
              color: '#71717a',
              lineWidth: 1,
              lineStyle: LineStyle.Dashed,
              axisLabelVisible: true,
              title: price.toString(),
            })
          }
        } else if (indicator.kind === 'macd' && variant === 'main') {
          lineSeries.createPriceLine({
            price: 0,
            color: '#71717a',
            lineWidth: 1,
            lineStyle: LineStyle.Dashed,
            axisLabelVisible: false,
            title: '',
          })
        }

        series.push(lineSeries)
      }
      indicatorSeriesMap.set(indicator.id, series)
    }

    updatePaneLayoutRef.current()
  }, [bars, indicators])
}
