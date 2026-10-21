import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type Timeframe,
  TimeframeSelector,
} from '#components/timeframe-selector'

afterEach(cleanup)

describe('TimeframeSelector default variant', () => {
  it('allows selecting each supported intraday interval', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: Timeframe) => void>()
    const intervals: Timeframe[] = ['1m', '5m', '15m', '1h', '4h']

    render(<TimeframeSelector value="1d" onChange={onChange} />)

    for (const interval of intervals) {
      await user.click(screen.getByRole('button', { name: interval }))
    }

    expect(onChange.mock.calls).toEqual(intervals.map((interval) => [interval]))
  })
})

describe('TimeframeSelector compact variant', () => {
  it('selects an additional timeframe', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: Timeframe) => void>()

    render(
      <TimeframeSelector variant="compact" value="1d" onChange={onChange} />,
    )

    await user.click(screen.getByRole('button', { name: 'その他の時間足' }))
    const menu = await screen.findByRole('listbox', { name: 'その他の時間足' })
    await user.click(within(menu).getByRole('option', { name: '5m' }))

    expect(onChange.mock.calls).toEqual([['5m']])
  })

  it('closes the menu after selecting an additional timeframe', async () => {
    const user = userEvent.setup()

    render(
      <TimeframeSelector
        variant="compact"
        value="1d"
        onChange={() => undefined}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'その他の時間足' }))
    const menu = await screen.findByRole('listbox', { name: 'その他の時間足' })
    await user.click(within(menu).getByRole('option', { name: '5m' }))

    expect(screen.queryByRole('listbox', { name: 'その他の時間足' })).toBeNull()
  })

  it('selects the weekly timeframe from its menu', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: Timeframe) => void>()

    render(
      <TimeframeSelector
        variant="compact"
        value="1d"
        onChange={onChange}
        moreTimeframesDefaultOpen
      />,
    )
    const menu = screen.getByRole('listbox', { name: 'その他の時間足' })
    await user.click(within(menu).getByRole('option', { name: '1W' }))

    expect(onChange.mock.calls).toEqual([['1w']])
  })
})
