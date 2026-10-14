import { RouterProvider } from '@tanstack/react-router'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { GroupDetailPage } from '#components/group-detail-page'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

const group: components['schemas']['Group'] = {
  key: 'demo-group',
  name: '見本グループ',
  description: '架空の分類に含まれる銘柄をまとめています。',
  axis: { key: 'demo-axis', name: '見本分類' },
  stocks: [
    { id: 'DEMO-A', name: '架空電機' },
    { id: 'DEMO-B', name: '見本素材' },
  ],
}

function getGroupDetailPageOutput() {
  return {
    stockRows: Array.from(document.querySelectorAll('tbody tr')).map((row) => {
      const stockLink = row.querySelector('a[href^="/stocks/"]')
      const holding = row.querySelector('td:nth-child(2)')?.textContent ?? ''

      return {
        name: stockLink?.textContent,
        href: stockLink?.getAttribute('href'),
        holding: holding.trim(),
      }
    }),
    headings: Array.from(document.querySelectorAll('h1, h2')).map(
      (heading) => heading.textContent,
    ),
  }
}

describe('GroupDetailPage', () => {
  it('links every stock and marks only held stocks', async () => {
    const router = createStoryRouter(
      () => (
        <GroupDetailPage
          state="loaded"
          group={group}
          heldStockIds={['DEMO-A']}
          isHoldingsLoading={false}
          isHoldingsError={false}
          notes={[]}
          isNotesError={false}
        />
      ),
      { paths: ['/stocks/$stockId', '/notes', '/notes/$noteId'] },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: '見本グループ' })

    expect(getGroupDetailPageOutput()).toEqual({
      stockRows: [
        {
          name: '架空電機',
          href: '/stocks/DEMO-A',
          holding: '保有',
        },
        {
          name: '見本素材',
          href: '/stocks/DEMO-B',
          holding: '',
        },
      ],
      headings: ['見本グループ', '>銘柄', '>関連ノート'],
    })
  })
})
