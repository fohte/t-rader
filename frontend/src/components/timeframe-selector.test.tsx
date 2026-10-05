import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { TimeframeSelector } from '#components/timeframe-selector'

describe('TimeframeSelector', () => {
  it('allows selecting every supported intraday interval', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()
    const intervals = ['1m', '5m', '15m', '1h', '4h'] as const

    render(<TimeframeSelector value="1d" onChange={onChange} />)

    for (const interval of intervals) {
      await user.click(screen.getByRole('button', { name: interval }))
    }

    expect(onChange.mock.calls).toEqual(intervals.map((interval) => [interval]))
  })
})
