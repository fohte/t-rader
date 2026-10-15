import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  addChartIndicator,
  type ChartIndicatorConfig,
  getChartIndicatorLookbackBars,
  getChartIndicatorPaneCount,
  removeChartIndicator,
  updateChartIndicatorDeviation,
  updateChartIndicatorMacdPeriod,
  updateChartIndicatorPeriod,
  useChartIndicators,
} from '#lib/chart-indicator-settings'

const STORAGE_KEY = 't-rader:chart-indicators'

function normalizeIds(indicators: ChartIndicatorConfig[]) {
  return indicators.map((indicator, index) => ({
    ...indicator,
    id: `indicator-${index.toString()}`,
  }))
}

beforeEach(() => {
  window.localStorage.clear()
})

afterEach(() => {
  cleanup()
})

describe('chart indicator settings', () => {
  it('adds and persists a default simple moving average', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('sma')
    })

    const persisted = (window.localStorage.getItem(STORAGE_KEY) ?? '').replace(
      indicatorId,
      'indicator-0',
    )
    const readSettingsState = () => ({
      current: normalizeIds(result.current),
      persisted,
    })

    expect(readSettingsState()).toEqual({
      current: [
        {
          id: 'indicator-0',
          kind: 'sma',
          period: 25,
          color: '#f97316',
        },
      ],
      persisted: JSON.stringify([
        {
          id: 'indicator-0',
          kind: 'sma',
          period: 25,
          color: '#f97316',
        },
      ]),
    })
  })

  it('adds RSI and MACD with their default periods and persists them', () => {
    const { result } = renderHook(() => useChartIndicators())
    let rsiId = ''
    let macdId = ''
    act(() => {
      rsiId = addChartIndicator('rsi')
      macdId = addChartIndicator('macd')
    })

    const normalized = normalizeIds(result.current)
    const persisted = (window.localStorage.getItem(STORAGE_KEY) ?? '')
      .replace(rsiId, 'indicator-0')
      .replace(macdId, 'indicator-1')
    const readSettingsState = () => ({ normalized, persisted })

    expect(readSettingsState()).toEqual({
      normalized: [
        {
          id: 'indicator-0',
          kind: 'rsi',
          period: 14,
          color: '#f97316',
        },
        {
          id: 'indicator-1',
          kind: 'macd',
          fastPeriod: 12,
          slowPeriod: 26,
          signalPeriod: 9,
          color: '#38bdf8',
        },
      ],
      persisted: JSON.stringify([
        {
          id: 'indicator-0',
          kind: 'rsi',
          period: 14,
          color: '#f97316',
        },
        {
          id: 'indicator-1',
          kind: 'macd',
          fastPeriod: 12,
          slowPeriod: 26,
          signalPeriod: 9,
          color: '#38bdf8',
        },
      ]),
    })
  })

  it('updates the period of the selected indicator', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('ema')
    })
    act(() => {
      updateChartIndicatorPeriod(indicatorId, 18)
    })

    expect(normalizeIds(result.current)).toEqual([
      {
        id: 'indicator-0',
        kind: 'ema',
        period: 18,
        color: '#f97316',
      },
    ])
  })

  it('keeps the current period when an invalid period is submitted', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('sma')
    })
    act(() => {
      updateChartIndicatorPeriod(indicatorId, 501)
    })

    expect(normalizeIds(result.current)).toEqual([
      {
        id: 'indicator-0',
        kind: 'sma',
        period: 25,
        color: '#f97316',
      },
    ])
  })

  it('updates the deviation of a Bollinger band', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('bollinger')
    })
    act(() => {
      updateChartIndicatorDeviation(indicatorId, 2.5)
    })

    expect(normalizeIds(result.current)).toEqual([
      {
        id: 'indicator-0',
        kind: 'bollinger',
        period: 20,
        deviation: 2.5,
        color: '#f97316',
      },
    ])
  })

  it('updates and persists each MACD period', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('macd')
      updateChartIndicatorMacdPeriod(indicatorId, 'fastPeriod', 8)
      updateChartIndicatorMacdPeriod(indicatorId, 'slowPeriod', 20)
      updateChartIndicatorMacdPeriod(indicatorId, 'signalPeriod', 5)
    })

    const normalized = normalizeIds(result.current)
    const persisted = (window.localStorage.getItem(STORAGE_KEY) ?? '').replace(
      indicatorId,
      'indicator-0',
    )
    const readSettingsState = () => ({ normalized, persisted })

    expect(readSettingsState()).toEqual({
      normalized: [
        {
          id: 'indicator-0',
          kind: 'macd',
          fastPeriod: 8,
          slowPeriod: 20,
          signalPeriod: 5,
          color: '#f97316',
        },
      ],
      persisted: JSON.stringify([
        {
          id: 'indicator-0',
          kind: 'macd',
          fastPeriod: 8,
          slowPeriod: 20,
          signalPeriod: 5,
          color: '#f97316',
        },
      ]),
    })
  })

  it('keeps MACD periods valid when an update reverses the EMA order', () => {
    const { result } = renderHook(() => useChartIndicators())
    let indicatorId = ''
    act(() => {
      indicatorId = addChartIndicator('macd')
      updateChartIndicatorMacdPeriod(indicatorId, 'fastPeriod', 26)
    })

    expect(normalizeIds(result.current)).toEqual([
      {
        id: 'indicator-0',
        kind: 'macd',
        fastPeriod: 12,
        slowPeriod: 26,
        signalPeriod: 9,
        color: '#f97316',
      },
    ])
  })

  it('reads saved legacy overlays and lower-pane settings', () => {
    const savedIndicators: ChartIndicatorConfig[] = [
      { id: 'legacy-sma', kind: 'sma', period: 25, color: '#f97316' },
      { id: 'legacy-ema', kind: 'ema', period: 18, color: '#38bdf8' },
      {
        id: 'legacy-bollinger',
        kind: 'bollinger',
        period: 20,
        deviation: 2,
        color: '#34d399',
      },
      {
        id: 'saved-macd',
        kind: 'macd',
        fastPeriod: 12,
        slowPeriod: 26,
        signalPeriod: 9,
        color: '#c084fc',
      },
    ]
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(savedIndicators))
    const { result } = renderHook(() => useChartIndicators())

    expect(result.current).toEqual(savedIndicators)
  })

  it('removes only the selected indicator', () => {
    const { result } = renderHook(() => useChartIndicators())
    let firstId = ''
    act(() => {
      firstId = addChartIndicator('sma')
      addChartIndicator('ema')
    })
    act(() => {
      removeChartIndicator(firstId)
    })

    expect(normalizeIds(result.current)).toEqual([
      {
        id: 'indicator-0',
        kind: 'ema',
        period: 25,
        color: '#38bdf8',
      },
    ])
  })

  it('accepts indicator settings written by another tab', () => {
    const { result } = renderHook(() => useChartIndicators())
    act(() => {
      window.dispatchEvent(
        new StorageEvent('storage', {
          key: STORAGE_KEY,
          newValue: JSON.stringify([
            {
              id: 'indicator-external',
              kind: 'ema',
              period: 12,
              color: '#38bdf8',
            },
          ]),
        }),
      )
    })

    expect(result.current).toEqual([
      {
        id: 'indicator-external',
        kind: 'ema',
        period: 12,
        color: '#38bdf8',
      },
    ])
  })

  it('fetches enough history for RSI and MACD and counts their lower panes', () => {
    const indicators: ChartIndicatorConfig[] = [
      { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
      { id: 'indicator-rsi', kind: 'rsi', period: 14, color: '#38bdf8' },
      {
        id: 'indicator-macd',
        kind: 'macd',
        fastPeriod: 12,
        slowPeriod: 26,
        signalPeriod: 9,
        color: '#34d399',
      },
    ]

    const readSettingsState = () => ({
      lookbackBars: getChartIndicatorLookbackBars(indicators),
      paneCount: getChartIndicatorPaneCount(indicators),
    })

    expect(readSettingsState()).toEqual({ lookbackBars: 34, paneCount: 2 })
  })
})
