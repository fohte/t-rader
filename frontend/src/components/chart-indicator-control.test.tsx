import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ChartIndicatorControl } from '#components/chart-indicator-control'
import type { ChartIndicatorConfig } from '#lib/chart-indicator-settings'

const matchMediaDescriptor = Object.getOwnPropertyDescriptor(
  window,
  'matchMedia',
)

afterEach(() => {
  cleanup()
  if (matchMediaDescriptor == null) {
    Reflect.deleteProperty(window, 'matchMedia')
  } else {
    Object.defineProperty(window, 'matchMedia', matchMediaDescriptor)
  }
})

describe('ChartIndicatorControl', () => {
  it('adds the selected indicator', () => {
    const onAdd = vi.fn()

    render(
      <ChartIndicatorControl
        indicators={[]}
        layout="popover"
        onAdd={onAdd}
        onRemove={vi.fn()}
        onUpdateDeviation={vi.fn()}
        onUpdatePeriod={vi.fn()}
        open
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '+ EMA' }))

    expect(onAdd.mock.calls).toEqual([['ema']])
  })

  it('updates a period after the input is committed', () => {
    const onUpdatePeriod = vi.fn()
    const indicators: ChartIndicatorConfig[] = [
      { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
    ]

    render(
      <ChartIndicatorControl
        indicators={indicators}
        layout="popover"
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onUpdateDeviation={vi.fn()}
        onUpdatePeriod={onUpdatePeriod}
        open
      />,
    )
    const periodInput = screen.getByRole('spinbutton', {
      name: 'SMA 25の期間',
    })
    fireEvent.change(periodInput, { target: { value: '30' } })
    fireEvent.blur(periodInput)

    expect(onUpdatePeriod.mock.calls).toEqual([['indicator-sma', 30]])
  })

  it('updates a Bollinger deviation as it changes', () => {
    const onUpdateDeviation = vi.fn()
    const indicators: ChartIndicatorConfig[] = [
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
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onUpdateDeviation={onUpdateDeviation}
        onUpdatePeriod={vi.fn()}
        open
      />,
    )
    fireEvent.change(
      screen.getByRole('spinbutton', { name: 'ボリンジャー 20の標準偏差' }),
      { target: { value: '2.5' } },
    )

    expect(onUpdateDeviation.mock.calls).toEqual([['indicator-bollinger', 2.5]])
  })

  it('removes the selected indicator', () => {
    const onRemove = vi.fn()
    const indicators: ChartIndicatorConfig[] = [
      { id: 'indicator-sma', kind: 'sma', period: 25, color: '#f97316' },
    ]

    render(
      <ChartIndicatorControl
        indicators={indicators}
        layout="popover"
        onAdd={vi.fn()}
        onRemove={onRemove}
        onUpdateDeviation={vi.fn()}
        onUpdatePeriod={vi.fn()}
        open
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'SMA 25を削除' }))

    expect(onRemove.mock.calls).toEqual([['indicator-sma']])
  })

  it('opens the menu as a bottom sheet on a mobile viewport', () => {
    const eventTarget = new EventTarget()
    const mediaQuery: MediaQueryList = {
      matches: true,
      media: '(max-width: 639px)',
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: eventTarget.addEventListener.bind(eventTarget),
      removeEventListener: eventTarget.removeEventListener.bind(eventTarget),
      dispatchEvent: (event) => eventTarget.dispatchEvent(event),
    }
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn(() => mediaQuery),
    })

    render(
      <ChartIndicatorControl
        indicators={[]}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onUpdateDeviation={vi.fn()}
        onUpdatePeriod={vi.fn()}
        defaultOpen
      />,
    )

    expect(screen.getByRole('heading', { name: '指標' }).textContent).toEqual(
      '指標',
    )
  })
})
