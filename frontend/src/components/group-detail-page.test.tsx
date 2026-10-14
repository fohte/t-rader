import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

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

afterEach(cleanup)

function renderLoadedGroupDetailPage({
  isHoldingsLoading = false,
  isHoldingsError = false,
}: {
  isHoldingsLoading?: boolean
  isHoldingsError?: boolean
} = {}) {
  const router = createStoryRouter(
    () => (
      <GroupDetailPage
        state="loaded"
        group={group}
        heldStockIds={['DEMO-A']}
        isHoldingsLoading={isHoldingsLoading}
        isHoldingsError={isHoldingsError}
        notes={[]}
        isNotesError={false}
      />
    ),
    { paths: ['/stocks/$stockId', '/notes', '/notes/$noteId'] },
  )
  render(<RouterProvider router={router} />)
  return screen.findByRole('heading', { name: '見本グループ' })
}

function getGroupDetailPageOutput() {
  return {
    stockColumns: Array.from(document.querySelectorAll('thead th')).map(
      (heading) => heading.textContent,
    ),
    stockRows: Array.from(document.querySelectorAll('tbody tr')).map((row) => {
      const cell = row.querySelector('td')
      const stockLink = cell?.querySelector('a[href^="/stocks/"]')
      const holdingBadge = Array.from(
        cell?.querySelectorAll('span') ?? [],
      ).find((span) => span.textContent === '保有')

      return {
        name: stockLink?.textContent ?? null,
        href: stockLink?.getAttribute('href') ?? null,
        holding: holdingBadge?.textContent ?? null,
      }
    }),
    holdingsStatus: Array.from(
      document.querySelectorAll('[role="status"]'),
    ).map((status) => status.textContent),
    headings: Array.from(document.querySelectorAll('h1, h2')).map(
      (heading) => heading.textContent,
    ),
  }
}

describe('GroupDetailPage', () => {
  it('links every stock and marks only held stocks', async () => {
    await renderLoadedGroupDetailPage()

    expect(getGroupDetailPageOutput()).toEqual({
      stockColumns: ['銘柄'],
      stockRows: [
        {
          name: '架空電機',
          href: '/stocks/DEMO-A',
          holding: '保有',
        },
        {
          name: '見本素材',
          href: '/stocks/DEMO-B',
          holding: null,
        },
      ],
      holdingsStatus: [],
      headings: ['見本グループ', '>銘柄', '>関連ノート'],
    })
  })

  it.each([
    {
      state: 'loading',
      isHoldingsLoading: true,
      isHoldingsError: false,
      status: '保有状況を読み込んでいます。',
    },
    {
      state: 'error',
      isHoldingsLoading: false,
      isHoldingsError: true,
      status: '保有状況を読み込めませんでした。',
    },
  ])(
    'does not show holding badges while holdings are $state',
    async ({ isHoldingsLoading, isHoldingsError, status }) => {
      await renderLoadedGroupDetailPage({ isHoldingsLoading, isHoldingsError })

      expect(getGroupDetailPageOutput()).toEqual({
        stockColumns: ['銘柄'],
        stockRows: [
          {
            name: '架空電機',
            href: '/stocks/DEMO-A',
            holding: null,
          },
          {
            name: '見本素材',
            href: '/stocks/DEMO-B',
            holding: null,
          },
        ],
        holdingsStatus: [status],
        headings: ['見本グループ', '>銘柄', '>関連ノート'],
      })
    },
  )
})
