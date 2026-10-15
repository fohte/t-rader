import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { StockChartPageView } from '#components/stock-page/stock-chart-page-view'
import { createStoryRouter } from '#storybook/story-router'

vi.mock('#components/candlestick-chart', () => ({
  CandlestickChart: () => null,
}))

afterEach(cleanup)

function renderChartPage(
  props: Partial<ComponentProps<typeof StockChartPageView>> = {},
) {
  const onCancelHorizontalLine = vi.fn()
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
        horizontalLineDrawingEnabled={false}
        pendingHorizontalLine={null}
        isSavingHorizontalLine={false}
        horizontalLineSaveError={null}
        onToggleHorizontalLineDrawing={() => undefined}
        onPlaceHorizontalLine={() => undefined}
        onSaveHorizontalLine={() => undefined}
        onCancelHorizontalLine={onCancelHorizontalLine}
        isDeletingAnnotation={false}
        annotationDeleteError={null}
        onDeleteAnnotation={() => undefined}
        currency="JPY"
        isLoading
        hasError={false}
        isAnnotationsLoading
        {...props}
      />
    ),
    { paths: ['/stocks/$stockId'] },
  )

  render(<RouterProvider router={router} />)
  return { onCancelHorizontalLine }
}

describe('StockChartPageView', () => {
  it('銘柄名の戻るリンクで銘柄ページを開く', async () => {
    renderChartPage()
    const backLink = await screen.findByRole('link', { name: '← デモ商事' })

    expect(
      Array.of(backLink.textContent, backLink.getAttribute('href')),
    ).toEqual(['← デモ商事', '/stocks/demo-code'])
  })

  it('水平線ツールのボタンで描画を開始する', async () => {
    const onToggleHorizontalLineDrawing = vi.fn()
    const user = userEvent.setup()
    renderChartPage({ onToggleHorizontalLineDrawing })

    await user.click(await screen.findByRole('button', { name: '水平線' }))

    expect(onToggleHorizontalLineDrawing.mock.calls).toEqual([[]])
  })

  it('Esc で描画中の水平線を取り消す', async () => {
    const user = userEvent.setup()
    const { onCancelHorizontalLine } = renderChartPage({
      horizontalLineDrawingEnabled: true,
    })

    await user.keyboard('{Escape}')

    expect(onCancelHorizontalLine.mock.calls).toEqual([[]])
  })

  it('Esc で種別キーとメモの入力中に水平線を取り消す', async () => {
    const user = userEvent.setup()
    const { onCancelHorizontalLine } = renderChartPage({
      pendingHorizontalLine: {
        timestamp: '2030-04-02T06:00:00Z',
        price: 3110,
      },
      isLoading: false,
    })

    await user.click(await screen.findByRole('combobox', { name: '種別キー' }))
    await user.keyboard('{Escape}')
    await user.click(await screen.findByRole('textbox', { name: 'メモ' }))
    await user.keyboard('{Escape}')

    expect(onCancelHorizontalLine.mock.calls).toEqual([[], []])
  })

  it('保存中は Esc で水平線を取り消さない', async () => {
    const user = userEvent.setup()
    const { onCancelHorizontalLine } = renderChartPage({
      pendingHorizontalLine: {
        timestamp: '2030-04-02T06:00:00Z',
        price: 3110,
      },
      isSavingHorizontalLine: true,
    })

    await user.keyboard('{Escape}')

    expect(onCancelHorizontalLine.mock.calls).toEqual([])
  })
})
