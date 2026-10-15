import { describe, expect, it } from 'vitest'

import {
  countActiveScreenerFilters,
  getScreenerRangeError,
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
  type SavedScreen,
  toSavedScreenRequest,
  toScreenerApiQuery,
  toScreenerSearch,
  validateScreenerSearch,
} from '#lib/screener-search'

describe('validateScreenerSearch', () => {
  it('normalizes supported URL values and drops invalid values', () => {
    expect(
      validateScreenerSearch({
        screen: 'saved-screen-a',
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
      screen: 'saved-screen-a',
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
      screen: undefined,
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

describe('saved screen search conversion', () => {
  const savedScreen: SavedScreen = {
    id: 'saved-screen-a',
    name: '試作条件',
    created_at: '2099-01-01T00:00:00Z',
    updated_at: '2099-01-02T00:00:00Z',
    filters: {
      markets: ['スタンダード', 'プライム'],
      sectors: ['sector-b', 'sector-a'],
      per: { min: null, max: 12 },
      pbr: { min: 0.5, max: 1 },
      roe: null,
      dividend_yield: { min: 3, max: null },
      market_cap: { min: 100, max: null },
    },
    sort: { field: 'per', direction: 'asc' },
  }

  it('converts an API screen to URL search values and resets paging', () => {
    expect(toScreenerSearch(savedScreen)).toEqual({
      markets: ['スタンダード', 'プライム'],
      sectors: ['sector-b', 'sector-a'],
      min_per: undefined,
      max_per: 12,
      min_pbr: 0.5,
      max_pbr: 1,
      min_roe: undefined,
      max_roe: undefined,
      min_dividend_yield: 3,
      max_dividend_yield: undefined,
      min_market_cap: 100,
      max_market_cap: undefined,
      sort_by: 'per',
      sort_order: 'asc',
      page: undefined,
    })
  })

  it('converts URL search values to a complete saved screen request', () => {
    expect(
      toSavedScreenRequest('  試作条件  ', {
        markets: ['プライム', 'スタンダード'],
        sectors: ['sector-b', 'sector-a'],
        min_per: 5,
        max_pbr: 1,
        min_roe: 8,
        min_dividend_yield: 3,
        min_market_cap: 100,
        sort_by: 'per',
        sort_order: 'asc',
      }),
    ).toEqual({
      name: '試作条件',
      filters: {
        markets: ['スタンダード', 'プライム'],
        sectors: ['sector-a', 'sector-b'],
        per: { min: 5, max: undefined },
        pbr: { min: undefined, max: 1 },
        roe: { min: 8, max: undefined },
        dividend_yield: { min: 3, max: undefined },
        market_cap: { min: 100, max: undefined },
      },
      sort: { field: 'per', direction: 'asc' },
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
          screen: 'saved-screen-a',
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
      screen: 'saved-screen-a',
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
          screen: 'saved-screen-a',
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
      screen: 'saved-screen-a',
      markets: ['プライム'],
      sectors: ['sector-a'],
      min_per: 5,
      sort_by: 'per',
      sort_order: 'asc',
      page: undefined,
    })
  })
})
