import { describe, expect, it } from 'vitest'

import {
  countActiveScreenerFilters,
  getScreenerRangeError,
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
  toScreenerApiQuery,
  validateScreenerSearch,
} from '#lib/screener-search'

describe('validateScreenerSearch', () => {
  it('normalizes supported URL values and drops invalid values', () => {
    expect(
      validateScreenerSearch({
        markets: ['プライム,グロース', '架空市場'],
        sectors: 'sector-a, sector-b',
        min_per: '8.5',
        max_per: '20',
        min_roe: '-5',
        sort_by: 'market_cap',
        sort_order: 'asc',
        page: '3',
        max_pbr: 'not-a-number',
      }),
    ).toEqual({
      markets: ['プライム', 'グロース'],
      sectors: ['sector-a', 'sector-b'],
      min_per: 8.5,
      max_per: 20,
      min_pbr: undefined,
      max_pbr: undefined,
      min_roe: -5,
      max_roe: undefined,
      min_dividend_yield: undefined,
      max_dividend_yield: undefined,
      min_market_cap: undefined,
      max_market_cap: undefined,
      sort_by: 'market_cap',
      sort_order: 'asc',
      page: 3,
    })
  })

  it('omits invalid sorting and page values', () => {
    expect(
      validateScreenerSearch({
        sort_by: 'unknown',
        sort_order: 'sideways',
        page: '0',
      }),
    ).toEqual({
      markets: undefined,
      sectors: undefined,
      min_per: undefined,
      max_per: undefined,
      min_pbr: undefined,
      max_pbr: undefined,
      min_roe: undefined,
      max_roe: undefined,
      min_dividend_yield: undefined,
      max_dividend_yield: undefined,
      min_market_cap: undefined,
      max_market_cap: undefined,
      sort_by: undefined,
      sort_order: undefined,
      page: undefined,
    })
  })
})

describe('toScreenerApiQuery', () => {
  it('serializes selections and keeps the requested page', () => {
    expect(
      toScreenerApiQuery(
        {
          markets: ['プライム', 'グロース'],
          sectors: ['sector-a'],
          min_per: 8,
          max_market_cap: 5000,
          sort_by: 'per',
          sort_order: 'asc',
        },
        2,
      ),
    ).toEqual({
      markets: 'プライム,グロース',
      sectors: 'sector-a',
      min_per: 8,
      max_per: undefined,
      min_pbr: undefined,
      max_pbr: undefined,
      min_roe: undefined,
      max_roe: undefined,
      min_dividend_yield: undefined,
      max_dividend_yield: undefined,
      min_market_cap: undefined,
      max_market_cap: 5000,
      sort_by: 'per',
      sort_order: 'asc',
      page: 2,
    })
  })
})

describe('screener filter helpers', () => {
  it('counts each active condition group once', () => {
    expect(
      countActiveScreenerFilters({
        markets: ['プライム', 'グロース'],
        sectors: ['sector-a'],
        min_per: 8,
        max_per: 20,
        max_roe: 10,
      }),
    ).toEqual(4)
  })

  it('returns an error when a range is reversed', () => {
    expect(getScreenerRangeError({ min_per: 20, max_per: 8 })).toEqual(
      '下限は上限以下にしてください。',
    )
  })

  it.each([
    [
      'new text column',
      {},
      'name',
      { sort_by: 'name', sort_order: 'asc', page: undefined },
    ],
    [
      'new numeric column',
      { sort_by: 'name', sort_order: 'asc' },
      'roe',
      { sort_by: 'roe', sort_order: 'desc', page: undefined },
    ],
    [
      'currently sorted column',
      { sort_by: 'per', sort_order: 'desc' },
      'per',
      { sort_by: 'per', sort_order: 'asc', page: undefined },
    ],
  ] as const)(
    'chooses the next direction for a %s',
    (_name, current, field, expected) => {
      expect(getScreenerSearchAfterSortBy(current, field)).toEqual(expected)
    },
  )
})

describe('screener sort search helpers', () => {
  it('preserves filters and resets the page when changing the sort field', () => {
    expect(
      getScreenerSearchAfterSortBy(
        {
          markets: ['プライム'],
          sectors: ['sector-a'],
          min_per: 5,
          sort_by: 'market_cap',
          sort_order: 'desc',
          page: 3,
        },
        'per',
      ),
    ).toEqual({
      markets: ['プライム'],
      sectors: ['sector-a'],
      min_per: 5,
      sort_by: 'per',
      sort_order: 'desc',
      page: undefined,
    })
  })

  it('preserves filters and resets the page when changing sort order', () => {
    expect(
      getScreenerSearchAfterSortOrder(
        {
          markets: ['プライム'],
          sectors: ['sector-a'],
          min_per: 5,
          sort_by: 'per',
          sort_order: 'desc',
          page: 3,
        },
        'asc',
      ),
    ).toEqual({
      markets: ['プライム'],
      sectors: ['sector-a'],
      min_per: 5,
      sort_by: 'per',
      sort_order: 'asc',
      page: undefined,
    })
  })
})
