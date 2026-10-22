import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'

afterEach(cleanup)

describe('ChartMarketDepthPanel', () => {
  it('isOpen が false のとき何も描画しない', () => {
    const { container } = render(
      <ChartMarketDepthPanel
        instrumentId="demo-code"
        isOpen={false}
        onToggle={vi.fn()}
      />,
    )

    expect(container.firstChild).toBeNull()
  })

  it('closes the panel when its close action is clicked', async () => {
    const onToggle = vi.fn()
    const user = userEvent.setup()

    render(
      <ChartMarketDepthPanel
        instrumentId="demo-code"
        isOpen={true}
        onToggle={onToggle}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'パネルを閉じる' }))

    expect(onToggle.mock.calls).toEqual([[]])
  })
})
