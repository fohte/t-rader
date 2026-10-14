import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockNameLink } from '#components/stocks/stock-name-link'
import { createStoryRouter } from '#storybook/story-router'

type Props = ComponentProps<typeof StockNameLink>

afterEach(cleanup)

async function renderStockNameLink(props: Props) {
  const router = createStoryRouter(() => <StockNameLink {...props} />, {
    paths: ['/stocks/$stockId'],
  })
  render(<RouterProvider router={router} />)
  await screen.findByText(
    props.name ??
      (props.missingName === 'link-symbol' ? props.symbol : '未解決'),
  )
}

function getLinkSnapshot() {
  const symbol = screen.getByText('demo-code')

  return {
    links: Array.from(document.querySelectorAll('a')).map((link) => [
      link.textContent,
      link.getAttribute('href'),
    ]),
    unresolvedName: screen.queryByText('未解決')?.textContent ?? null,
    symbolCount: screen.getAllByText('demo-code').length,
    symbolIsLinked: symbol.closest('a') != null,
  }
}

describe('StockNameLink', () => {
  it('リンク名が解決済みならコードを添えて名前だけをリンクにする', async () => {
    await renderStockNameLink({
      name: '架空商事',
      symbol: 'demo-code',
      missingName: 'link-symbol',
    })

    expect(getLinkSnapshot()).toEqual({
      links: [['架空商事', '/stocks/demo-code']],
      unresolvedName: null,
      symbolCount: 1,
      symbolIsLinked: false,
    })
  })

  it('リンク名が未登録ならコード自体をリンクにする', async () => {
    await renderStockNameLink({
      name: null,
      symbol: 'demo-code',
      missingName: 'link-symbol',
    })

    expect(getLinkSnapshot()).toEqual({
      links: [['demo-code', '/stocks/demo-code']],
      unresolvedName: null,
      symbolCount: 1,
      symbolIsLinked: true,
    })
  })

  it('未解決表示では銘柄名をリンクにしない', async () => {
    await renderStockNameLink({
      name: null,
      symbol: 'demo-code',
      missingName: 'unresolved',
    })

    expect(getLinkSnapshot()).toEqual({
      links: [],
      unresolvedName: '未解決',
      symbolCount: 1,
      symbolIsLinked: false,
    })
  })
})
