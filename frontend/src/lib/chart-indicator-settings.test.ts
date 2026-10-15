import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'

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
  return indicators.map((indicator) => ({ ...indicator, id: 'indicator-id' }))
}

beforeEach(() => {
  window.localStorage.clear()
})

describe('chart indicator settings', () => {
  it('persists defaults and updates while removing selected indicators', () => {
    const { result } = renderHook(() => useChartIndicators())
    act(() => {
      window.dispatchEvent(new StorageEvent('storage', { key: null }))
    })

    let bollingerId = ''
    let smaId = ''
    act(() => {
      bollingerId = addChartIndicator('bollinger')
    })
    const afterAdd = result.current

    act(() => {
      smaId = addChartIndicator('sma')
    })
    act(() => {
      updateChartIndicatorPeriod(bollingerId, 18)
      updateChartIndicatorDeviation(bollingerId, 2.5)
      updateChartIndicatorPeriod(bollingerId, 501)
      removeChartIndicator(smaId)
    })

    const persisted = (window.localStorage.getItem(STORAGE_KEY) ?? '').replace(
      bollingerId,
      'indicator-id',
    )

    const readSettingsState = () => ({
      afterAdd: normalizeIds(afterAdd),
      current: normalizeIds(result.current),
      persisted,
    })

    expect(readSettingsState()).toEqual({
      afterAdd: [
        {
          id: 'indicator-id',
          kind: 'bollinger',
          period: 20,
          deviation: 2,
          color: '#f97316',
        },
      ],
      current: [
        {
          id: 'indicator-id',
          kind: 'bollinger',
          period: 18,
          deviation: 2.5,
          color: '#f97316',
        },
      ],
      persisted: JSON.stringify([
        {
          id: 'indicator-id',
          kind: 'bollinger',
          period: 18,
          deviation: 2.5,
          color: '#f97316',
        },
      ]),
    })
  })
})
