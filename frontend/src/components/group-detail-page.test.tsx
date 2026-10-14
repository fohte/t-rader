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
    stockLinks: Array.from(
      document.querySelectorAll('a[href^="/stocks/"]'),
    ).map((link) => [link.textContent, link.getAttribute('href')]),
    heldBadges: screen.getAllByText('保有', { exact: true }).length,
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
          notes={[]}
          isNotesError={false}
        />
      ),
      { paths: ['/stocks/$stockId', '/notes', '/notes/$noteId'] },
    )
    render(<RouterProvider router={router} />)
    await screen.findByRole('heading', { name: '見本グループ' })

    expect(getGroupDetailPageOutput()).toEqual({
      stockLinks: [
        ['架空電機', '/stocks/DEMO-A'],
        ['見本素材', '/stocks/DEMO-B'],
      ],
      heldBadges: 1,
      headings: ['見本グループ', '>銘柄', '>関連ノート'],
    })
  })
})
