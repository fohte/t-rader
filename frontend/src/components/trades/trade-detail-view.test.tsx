import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { TradeDetailView } from '#components/trades/trade-detail-view'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type TradeDetail = components['schemas']['TradeDetailResponse']
type Note = components['schemas']['Note']

function readTradeDetailViewOutput(
  facts: string,
  noteLinks: Array<{
    title: string | undefined
    author: string | undefined
    href: string | null
  }>,
  stockLink: string | null | undefined,
) {
  return { facts, noteLinks, stockLink }
}

afterEach(cleanup)

describe('TradeDetailView', () => {
  it('shows sale facts, matched purchases, and note author links', async () => {
    const trade: TradeDetail = {
      id: 'fake-trade-sell',
      strategy_id: 'fake-strategy',
      symbol: 'FIC-01',
      side: 'sell',
      qty: 10,
      price: 1000,
      fee: 10,
      date: '2099-02-10',
      source: 'manual',
      note: null,
      created_at: '2099-02-10T00:00:00Z',
      updated_at: '2099-02-10T00:00:00Z',
      realized_pnl: 2200,
      matched_buys: [{ date: '2099-02-01', qty: 10, price: 780 }],
    }
    const notes: Note[] = [
      {
        id: 'fake-note-human',
        version_id: 'fake-note-version-human',
        version_no: 1,
        is_current: true,
        title: '架空銘柄を買った理由',
        body_md: '架空の内容です。',
        frontmatter_json: {},
        resolved_price_references_json: {},
        graphs_json: [],
        tags: [],
        kind: null,
        status: 'approved',
        trigger: null,
        trigger_label: null,
        created_by_kind: 'human',
        created_at: '2099-02-10T00:00:00Z',
        updated_at: '2099-02-10T00:00:00Z',
        execution_id: null,
      },
      {
        id: 'fake-note-agent',
        version_id: 'fake-note-version-agent',
        version_no: 1,
        is_current: true,
        title: '架空銘柄の観察メモ',
        body_md: '架空の内容です。',
        frontmatter_json: {},
        resolved_price_references_json: {},
        graphs_json: [],
        tags: [],
        kind: null,
        status: 'unread',
        trigger: null,
        trigger_label: null,
        created_by_kind: 'llm',
        created_at: '2099-02-10T00:00:00Z',
        updated_at: '2099-02-10T00:00:00Z',
        execution_id: null,
      },
    ]
    const router = createStoryRouter(
      () => (
        <TradeDetailView
          state="ready"
          trade={trade}
          chartState={{ status: 'loading' }}
          stockName="架空銘柄"
          strategyName="検証用戦略"
          linkedNotes={notes}
          notesPending={false}
          notesError={false}
          requestedAnalyses={[]}
          requestedAnalysesState="success"
          noteLinkError={null}
          onManageNotes={() => undefined}
          onWriteReflection={() => undefined}
        />
      ),
      {
        paths: [
          '/trades',
          '/trades/$tradeId',
          '/stocks/$stockId',
          '/notes/$noteId',
        ],
      },
    )
    const { container } = render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { level: 1 })

    const facts = screen.getByRole('region', { name: '>取引の事実' })
    const notesSection = screen.getByRole('region', {
      name: '>この取引のノート',
    })

    expect(
      readTradeDetailViewOutput(
        facts.textContent.replace(/\s/g, ''),
        Array.from(notesSection.querySelectorAll('a')).map((link) => ({
          title: link.firstElementChild?.textContent,
          author: link.lastElementChild?.textContent,
          href: link.getAttribute('href'),
        })),
        container.querySelector('h1 a')?.getAttribute('href'),
      ),
    ).toEqual({
      facts:
        '>取引の事実売買売り数量10株単価¥1,000実現損益+¥2,200対応する買い2099-02-0110株¥780',
      noteLinks: [
        {
          title: '架空銘柄を買った理由',
          author: '自分',
          href: '/notes/fake-note-human?version_id=fake-note-version-human',
        },
        {
          title: '架空銘柄の観察メモ',
          author: 'エージェント',
          href: '/notes/fake-note-agent?version_id=fake-note-version-agent',
        },
      ],
      stockLink: '/stocks/FIC-01',
    })
  })
})
