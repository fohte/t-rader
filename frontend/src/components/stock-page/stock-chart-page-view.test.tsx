import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('StockChartPageView', () => {
  it('銘柄名の戻るリンクで銘柄ページを開く', async () => {
    const router = createStoryRouter(
      () => (
        <StockChartPageView
          stockId="demo-code"
          stockName="デモ商事"
          bars={[]}
          annotations={[]}
          timeframe="1d"
          onTimeframeChange={() => undefined}
          isMarketDepthOpen={false}
          onToggleMarketDepth={() => undefined}
          selectedAnnotationId={null}
          onSelectAnnotation={() => undefined}
          currency="JPY"
          isLoading
          hasError={false}
          isAnnotationsLoading
        />
      ),
      { paths: ['/stocks/$stockId'] },
    )

    render(<RouterProvider router={router} />)
    const backLink = await screen.findByRole('link', { name: '← デモ商事' })

    expect(
      Array.of(backLink.textContent, backLink.getAttribute('href')),
    ).toEqual(['← デモ商事', '/stocks/demo-code'])
  })
})
