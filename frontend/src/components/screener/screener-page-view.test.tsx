import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ScreenerPageView } from '#components/screener/screener-page-view'
import type { components } from '#lib/api/schema.gen'

type ScreenerResponse = components['schemas']['ScreenerResponse']

const data: ScreenerResponse = {
  items: [],
  total_count: 0,
  as_of: '2099-06-18',
  page: 1,
}

afterEach(cleanup)

describe('ScreenerPageView', () => {
  it('applies all selected screener conditions together', async () => {
    const user = userEvent.setup()
    const onApplyFilters = vi.fn()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [{ key: 'sector-sample', name: 'サンプル業' }],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      loadMoreError: false,
      onApplyFilters,
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    render(<ScreenerPageView {...args} />)

    await user.click(screen.getByRole('checkbox', { name: 'プライム' }))
    await user.click(screen.getByRole('checkbox', { name: 'サンプル業' }))

    const ranges = [
      ['PER 下限', '8'],
      ['PER 上限', '15'],
      ['PBR 下限', '0.5'],
      ['PBR 上限', '2'],
      ['ROE 下限', '6'],
      ['ROE 上限', '20'],
      ['配当利回り (予) 下限', '2'],
      ['配当利回り (予) 上限', '5'],
      ['時価総額 下限', '500'],
      ['時価総額 上限', '10000'],
    ] as const
    for (const [label, value] of ranges) {
      await user.type(screen.getByRole('spinbutton', { name: label }), value)
    }

    await user.click(screen.getByRole('button', { name: 'この条件で絞り込む' }))

    expect(onApplyFilters.mock.calls).toEqual([
      [
        {
          markets: ['プライム'],
          sectors: ['sector-sample'],
          min_per: 8,
          max_per: 15,
          min_pbr: 0.5,
          max_pbr: 2,
          min_roe: 6,
          max_roe: 20,
          min_dividend_yield: 2,
          max_dividend_yield: 5,
          min_market_cap: 500,
          max_market_cap: 10000,
        },
      ],
    ])
  })

  it('keeps an unapplied filter draft when sorting and paging change', async () => {
    const user = userEvent.setup()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      onApplyFilters: () => {},
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    const { rerender } = render(<ScreenerPageView {...args} />)
    const perLowerBound = screen.getByRole<HTMLInputElement>('spinbutton', {
      name: 'PER 下限',
    })
    await user.type(perLowerBound, '12')
    rerender(
      <ScreenerPageView
        {...args}
        search={{ sort_by: 'per', sort_order: 'asc', page: 2 }}
      />,
    )

    expect(perLowerBound.value).toBe('12')
  })

  it('shows reversed range validation and does not apply invalid filters', async () => {
    const user = userEvent.setup()
    const onApplyFilters = vi.fn()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      onApplyFilters,
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    render(<ScreenerPageView {...args} />)
    await user.type(screen.getByRole('spinbutton', { name: 'PER 下限' }), '20')
    await user.type(screen.getByRole('spinbutton', { name: 'PER 上限' }), '8')
    await user.click(screen.getByRole('button', { name: 'この条件で絞り込む' }))

    const actual = JSON.stringify({
      error: screen.getByRole('alert').textContent,
      calls: onApplyFilters.mock.calls,
    })
    const expected = JSON.stringify({
      error: '下限は上限以下にしてください。',
      calls: [],
    })
    expect(actual).toBe(expected)
  })
})
