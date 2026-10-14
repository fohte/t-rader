import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockPageChart } from '#components/stock-page/stock-page-chart'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('StockPageChart', () => {
  it('全画面リンクで銘柄のチャートを開く', async () => {
    const router = createStoryRouter(
      () => (
        <StockPageChart
          stockId="demo-code"
          bars={[]}
          annotations={[]}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
          timeframe="1d"
          onTimeframeChange={() => undefined}
          isLoading
        />
      ),
      { paths: ['/stocks/$stockId/chart'] },
    )

    render(<RouterProvider router={router} />)
    const fullScreenLink = await screen.findByRole('link', {
      name: '全画面チャートを開く',
    })

    expect(
      Array.of(fullScreenLink.textContent, fullScreenLink.getAttribute('href')),
    ).toEqual(['全画面 ↗', '/stocks/demo-code/chart'])
  })
})
