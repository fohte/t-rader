import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import {
  addChartIndicator,
  type ChartIndicatorConfig,
  removeChartIndicator,
  updateChartIndicatorDeviation,
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
})
