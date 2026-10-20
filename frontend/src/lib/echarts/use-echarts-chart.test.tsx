import { cleanup, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { useEChartsChart } from '#lib/echarts/use-echarts-chart'

const chartHarness = vi.hoisted(() => ({
  init: vi.fn(),
}))

vi.mock('echarts/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('echarts/core')>()
  return {
    ...actual,
    init: chartHarness.init,
  }
})

afterEach(() => {
  cleanup()
})

beforeEach(() => {
  chartHarness.init.mockReset()
  chartHarness.init.mockImplementation(() => {
    throw new Error('架空の初期化エラー')
  })
})

describe('useEChartsChart', () => {
  it('初期化に失敗した場合は描画エラーを返す', async () => {
    function HookHarness() {
      const { containerRef, error } = useEChartsChart({
        createOption: () => ({}),
      })
      return (
        <div ref={containerRef} role={error == null ? undefined : 'alert'}>
          {error}
        </div>
      )
    }

    const rendered = render(<HookHarness />)
    const readOutput = () => ({
      error: rendered.container.querySelector('[role="alert"]')?.textContent,
      initCalls: chartHarness.init.mock.calls.length,
    })

    await waitFor(() => {
      expect(readOutput()).toEqual({
        error: 'ECharts の描画に失敗しました。',
        initCalls: 1,
      })
    })
  })
})
