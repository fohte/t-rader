import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ScreenerResultsView } from '#components/screener/screener-results-view'
import type { components } from '#lib/api/schema.gen'
import type {
  ScreenerSearch,
  ScreenerSortBy,
  ScreenerSortOrder,
} from '#lib/screener-search'
import {
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
} from '#lib/screener-search'
import { createStoryRouter } from '#storybook/story-router'

type ScreenerResponse = components['schemas']['ScreenerResponse']
type ScreenerStock = components['schemas']['ScreenerStockResponse']

const stocks: ScreenerStock[] = [
  {
    id: 'demo-code',
    name: '架空商事',
    market: 'プライム',
    sector: 'サンプル業',
    close: 3250,
    per: 8.2,
    pbr: 0.7,
    roe: 9.1,
    dividend_yield: 3.8,
    market_cap: 5140,
  },
]

const data: ScreenerResponse = {
  items: stocks,
  total_count: 1,
  as_of: '2099-06-18',
  page: 1,
}

afterEach(cleanup)

describe('ScreenerResultsView', () => {
  it('updates the selected sort field and direction for the URL state', async () => {
    const user = userEvent.setup()
    const initialSearch: ScreenerSearch = {
      markets: ['プライム'],
      sectors: ['sector-a'],
      min_per: 5,
      sort_by: 'market_cap',
      sort_order: 'desc',
      page: 3,
    }
    let nextSearch = initialSearch
    const onSort = vi.fn((field: ScreenerSortBy) => {
      nextSearch = getScreenerSearchAfterSortBy(nextSearch, field)
    })
    const onSortOrder = vi.fn((order: ScreenerSortOrder) => {
      nextSearch = getScreenerSearchAfterSortOrder(nextSearch, order)
    })
    const args = {
      search: initialSearch,
      data,
      stocks,
      status: 'results',
      onSort,
      onSortOrder,
      onLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerResultsView>
    const router = createStoryRouter(() => <ScreenerResultsView {...args} />, {
      paths: ['/stocks/$stockId'],
    })

    render(<RouterProvider router={router} />)

    const sortField = await screen.findByRole('combobox', {
      name: '並べ替える列',
    })
    const sortOrder = await screen.findByRole('combobox', {
      name: '並べ替え順',
    })
    await user.selectOptions(sortField, 'per')
    await user.selectOptions(sortOrder, 'asc')

    expect(nextSearch).toEqual({
      markets: ['プライム'],
      sectors: ['sector-a'],
      min_per: 5,
      sort_by: 'per',
      sort_order: 'asc',
      page: undefined,
    })
  })
})
