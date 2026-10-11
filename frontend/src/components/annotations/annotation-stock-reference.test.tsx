import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { AnnotationStockReference } from '#components/annotations/annotation-stock-reference'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

async function renderReference(name: string | null) {
  const router = createStoryRouter(
    () => <AnnotationStockReference name={name} symbol="demo-code" />,
    { paths: ['/charts/$instrumentId'] },
  )
  render(<RouterProvider router={router} />)
  await screen.findByText(name ?? '未解決')
}

function getReferenceSnapshot() {
  const link = screen.queryByRole('link')
  const unresolved = screen.queryByText('未解決')
  const symbol = screen.getByText('demo-code')

  return {
    linkCount: screen.queryAllByRole('link').length,
    name: link?.textContent ?? unresolved?.textContent ?? null,
    href: link?.getAttribute('href') ?? null,
    symbol: symbol.textContent,
    symbolIsLinked: symbol.closest('a') != null,
  }
}

describe('AnnotationStockReference', () => {
  it('解決済みの銘柄名だけを日足チャートへのリンクとして表示する', async () => {
    await renderReference('架空商事')

    expect(getReferenceSnapshot()).toEqual({
      linkCount: 1,
      name: '架空商事',
      href: '/charts/demo-code',
      symbol: 'demo-code',
      symbolIsLinked: false,
    })
  })

  it('未解決の銘柄名はリンクにせずコードを表示する', async () => {
    await renderReference(null)

    expect(getReferenceSnapshot()).toEqual({
      linkCount: 0,
      name: '未解決',
      href: null,
      symbol: 'demo-code',
      symbolIsLinked: false,
    })
  })
})
