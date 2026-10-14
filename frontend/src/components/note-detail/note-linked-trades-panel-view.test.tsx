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

function readPanelOutput(panel: HTMLElement, container: HTMLElement) {
  const row = panel.querySelector('li')

  return {
    heading: panel.querySelector('h2')?.textContent,
    text: row?.textContent.replace(/\s+/g, ' ').trim(),
    links: Array.from(row?.querySelectorAll('a') ?? []).map((link) => ({
      text: link.textContent.trim(),
      href: link.getAttribute('href'),
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
          trades={[linkedTrade]}
          stockNameById={new Map([['FIC-01', '架空商事']])}
        />
      ),
      { paths: ['/trades/$tradeId', '/stocks/$stockId'] },
    )
    const { container } = render(<RouterProvider router={router} />)
    const panel = await screen.findByRole('region', { name: '紐付いた取引' })

    expect(readPanelOutput(panel, container)).toEqual({
      heading: '紐付いた取引',
      text: '2099-02-10買い詳細 →架空商事FIC-01',
      links: [
        { text: '2099-02-10買い', href: '/trades/fake-trade-buy' },
        { text: '詳細 →', href: '/trades/fake-trade-buy' },
        { text: '架空商事', href: '/stocks/FIC-01' },
      ],
      tradeCount: 1,
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
