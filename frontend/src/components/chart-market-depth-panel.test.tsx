import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { ChartMarketDepthPanel } from '#components/chart-market-depth-panel'

describe('ChartMarketDepthPanel', () => {
  it('isOpen が false のとき何も描画しない', () => {
    render(
      <ChartMarketDepthPanel
        instrumentId="7203"
        isOpen={false}
        onToggle={vi.fn()}
      />,
    )

    expect(screen.queryByText('板情報・歩み値')).not.toBeInTheDocument()
  })
})
