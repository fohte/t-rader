import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ChartIndicatorControl } from '#components/chart-indicator-control'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

describe('ChartIndicatorControl', () => {
  it('adds, edits, and removes chart indicators', async () => {
    const user = userEvent.setup()
    const onAdd = vi.fn()
    const onUpdatePeriod = vi.fn()
    const onUpdateDeviation = vi.fn()
    const onRemove = vi.fn()
    const indicators: ChartIndicatorConfig[] = [
      { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
      {
        id: 'indicator-bollinger',
        kind: 'bollinger',
        period: 20,
        deviation: 2,
        color: '#38bdf8',
      },
    ]

    render(
      <ChartIndicatorControl
        indicators={indicators}
        layout="popover"
        onAdd={onAdd}
        onRemove={onRemove}
        onUpdateDeviation={onUpdateDeviation}
        onUpdatePeriod={onUpdatePeriod}
        open
      />,
    )

    await user.click(screen.getByRole('button', { name: '+ EMA' }))
    fireEvent.change(screen.getByRole('spinbutton', { name: 'SMA 25の期間' }), {
      target: { value: '30' },
    })
    fireEvent.change(
      screen.getByRole('spinbutton', { name: 'ボリンジャー 20の標準偏差' }),
      { target: { value: '2.5' } },
    )
    await user.click(screen.getByRole('button', { name: 'SMA 25を削除' }))

    const readActionCalls = () => ({
      add: onAdd.mock.calls,
      updatePeriod: onUpdatePeriod.mock.calls,
      updateDeviation: onUpdateDeviation.mock.calls,
      remove: onRemove.mock.calls,
    })

    expect(readActionCalls()).toEqual({
      add: [['ema']],
      updatePeriod: [['indicator-sma', 30]],
      updateDeviation: [['indicator-bollinger', 2.5]],
      remove: [['indicator-sma']],
    })
  })
})
