import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StockPageChart } from '#components/stock-page/stock-page-chart'
import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']

vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: ({
    highlightedTimestamp,
  }: {
    highlightedTimestamp?: string
  }) => (
    <div aria-label="日足チャート" role="img">
      {highlightedTimestamp}
    </div>
  ),
}))

const bar: Bar = {
  close: 3060,
  high: 3100,
  instrument_id: 'demo-code',
  low: 2980,
  open: 3000,
  timeframe: '1d',
  timestamp: '2030-04-02T15:00:00Z',
  volume: 10000,
}

afterEach(() => {
  cleanup()
})

describe('StockPageChart', () => {
  it.each([
    [
      'a date with a daily bar',
      '2030-04-03',
      bar,
      '2030-04-03 始 3,000 高 3,100 安 2,980 終 3,060',
      bar.timestamp,
    ],
    [
      'a date without a daily bar',
      '2030-04-04',
      null,
      '2030-04-04 の日足データがありません',
      '',
    ],
  ])(
    'shows the selected-date state for %s',
    (
      _name,
      selectedDate,
      selectedDateBar,
      expectedSummary,
      expectedTimestamp,
    ) => {
      render(
        <StockPageChart
          bars={[bar]}
          annotations={[]}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
          timeframe="1d"
          onTimeframeChange={() => undefined}
          selectedDate={selectedDate}
          selectedDateBar={selectedDateBar}
        />,
      )

      const readRenderedState = () => ({
        summary: screen.getByRole('status').getAttribute('aria-label'),
        chartTimestamp: screen.getByRole('img', { name: '日足チャート' })
          .textContent,
      })

      expect(readRenderedState()).toEqual({
        summary: expectedSummary,
        chartTimestamp: expectedTimestamp,
      })
    },
  )
})
