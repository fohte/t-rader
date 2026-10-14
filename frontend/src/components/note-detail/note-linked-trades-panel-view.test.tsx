import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteLinkedTradesPanelView } from '#components/note-detail/note-linked-trades-panel-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type NoteLinkedTrade = components['schemas']['NoteLinkedTrade']

const linkedTrade: NoteLinkedTrade = {
  id: 'fake-trade-buy',
  date: '2099-02-10',
  symbol: 'FIC-01',
  side: 'buy',
}

const linkedSellTrade: NoteLinkedTrade = {
  id: 'fake-trade-sell',
  date: '2099-02-11',
  symbol: 'FIC-02',
  side: 'sell',
}

function readPanelOutput(panel: HTMLElement, container: HTMLElement) {
  return {
    heading: panel.querySelector('h2')?.textContent,
    rows: Array.from(panel.querySelectorAll('li')).map((row) => ({
      text: row.textContent.replace(/\s+/g, ' ').trim(),
      links: Array.from(row.querySelectorAll('a')).map((link) => ({
        text: link.textContent.trim(),
        href: link.getAttribute('href'),
      })),
    })),
    tradeCount: container.querySelectorAll('li').length,
  }
}

afterEach(cleanup)

describe('NoteLinkedTradesPanelView', () => {
  it('shows each linked trade and links to its details and stock page', async () => {
    const router = createStoryRouter(
      () => (
        <NoteLinkedTradesPanelView
          state="ready"
          trades={[linkedTrade, linkedSellTrade]}
          stockNameById={new Map([['FIC-01', '架空商事']])}
        />
      ),
      { paths: ['/trades/$tradeId', '/stocks/$stockId'] },
    )
    const { container } = render(<RouterProvider router={router} />)
    const panel = await screen.findByRole('region', { name: '紐付いた取引' })

    expect(readPanelOutput(panel, container)).toEqual({
      heading: '紐付いた取引',
      rows: [
        {
          text: '2099-02-10買い詳細 →架空商事FIC-01',
          links: [
            { text: '2099-02-10買い', href: '/trades/fake-trade-buy' },
            { text: '詳細 →', href: '/trades/fake-trade-buy' },
            { text: '架空商事', href: '/stocks/FIC-01' },
          ],
        },
        {
          text: '2099-02-11売り詳細 →FIC-02',
          links: [
            { text: '2099-02-11売り', href: '/trades/fake-trade-sell' },
            { text: '詳細 →', href: '/trades/fake-trade-sell' },
            { text: 'FIC-02', href: '/stocks/FIC-02' },
          ],
        },
      ],
      tradeCount: 2,
    })
  })

  it('hides the panel when no trades are linked', () => {
    const router = createStoryRouter(
      () => (
        <NoteLinkedTradesPanelView
          state="ready"
          trades={[]}
          stockNameById={new Map()}
        />
      ),
      { paths: ['/trades/$tradeId', '/stocks/$stockId'] },
    )

    const { container } = render(<RouterProvider router={router} />)

    expect(container.firstChild).toBeNull()
  })
})
