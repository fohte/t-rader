import { describe, expect, it } from 'vitest'

import {
  buildScreenerAgentRequest,
  clearSavedScreenSelectionSearch,
  countActiveScreenerFilters,
  getScreenerRangeError,
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
  isSavedScreenSelectionOnly,
  restoreSavedScreenSearch,
  type SavedScreen,
  savedScreenHasSearchChanges,
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

  it('restores a saved screen selection with its filters and sort order', () => {
    expect(
      restoreSavedScreenSearch({ screen: savedScreen.id }, [savedScreen]),
    ).toEqual({
      ...toScreenerSearch(savedScreen),
      screen: savedScreen.id,
    })
  })

  it('does not replace an explicit search with saved screen values', () => {
    expect(
      restoreSavedScreenSearch({ screen: savedScreen.id, max_per: 10 }, [
        savedScreen,
      ]),
    ).toEqual(undefined)
  })

  it('treats reordered filters and omitted defaults as the saved conditions', () => {
    const savedSearch = toScreenerSearch(savedScreen)

    expect(
      savedScreenHasSearchChanges(savedScreen, {
        ...savedSearch,
        screen: savedScreen.id,
        markets: [...(savedSearch.markets ?? [])].reverse(),
        sectors: [...(savedSearch.sectors ?? [])].reverse(),
        page: 2,
      }),
    ).toEqual(false)
  })

  it.each([
    ['filter values', { ...toScreenerSearch(savedScreen), max_per: 11 }],
    ['sort values', { ...toScreenerSearch(savedScreen), sort_order: 'desc' }],
  ] as const)('detects changed %s', (_name, search) => {
    expect(savedScreenHasSearchChanges(savedScreen, search)).toEqual(true)
  })
})

describe('saved screen selection URL', () => {
  it('recognizes a URL that contains only a saved screen id', () => {
    expect(
      isSavedScreenSelectionOnly({ screen: 'saved-screen-a', page: 2 }),
    ).toEqual(true)
  })
})

describe('buildScreenerAgentRequest', () => {
  it('references an unchanged saved screen by name and id', () => {
    const savedScreen: SavedScreen = {
      id: 'fake-screen-a',
      name: '架空条件',
      created_at: '2099-01-01T00:00:00Z',
      updated_at: '2099-01-02T00:00:00Z',
      filters: {
        markets: ['プライム'],
        sectors: [],
        per: { min: null, max: 12 },
        pbr: null,
        roe: null,
        dividend_yield: null,
        market_cap: null,
      },
      sort: { field: 'per', direction: 'asc' },
    }

    expect(
      buildScreenerAgentRequest(
        {
          screen: savedScreen.id,
          markets: ['プライム'],
          max_per: 12,
          sort_by: 'per',
          sort_order: 'asc',
        },
        savedScreen,
      ),
    ).toEqual(
      '保存した条件「架空条件」(screen_id: fake-screen-a) で銘柄候補を探してください。',
    )
  })

  it('writes current filters, units, and sort order when the selection changed', () => {
    expect(
      buildScreenerAgentRequest(
        {
          markets: ['プライム'],
          sectors: ['sector-a'],
          min_per: 8,
          max_pbr: 1,
          min_roe: 6,
          min_dividend_yield: 3,
          max_market_cap: 1200,
          sort_by: 'per',
          sort_order: 'asc',
        },
        undefined,
        [{ key: 'sector-a', name: '架空業種' }],
      ),
    ).toEqual(
      [
        '次の条件と並び順で銘柄候補を探してください。',
        '条件: 市場: プライム、業種: 架空業種、PER: 8以上倍、PBR: 1以下倍、ROE: 6以上%、配当利回り (予): 3以上%、時価総額: 1200以下億円',
        '並び順: PER (昇順)',
      ].join('\n'),
    )
  })

  it('writes an unrestricted search and default sort when no filters are set', () => {
    expect(buildScreenerAgentRequest({})).toEqual(
      [
        '次の条件と並び順で銘柄候補を探してください。',
        '条件: なし (全銘柄)',
        '並び順: 時価総額 (降順)',
      ].join('\n'),
    )
  })
})

describe('saved screen URL selection', () => {
  it('clears only the selected screen while keeping the current search', () => {
    expect(
      clearSavedScreenSelectionSearch({
        screen: 'missing-screen',
        markets: ['プライム'],
        sectors: ['sector-a'],
        max_per: 12,
        sort_by: 'per',
        sort_order: 'asc',
        page: 2,
      }),
    ).toEqual({
      screen: undefined,
      markets: ['プライム'],
      sectors: ['sector-a'],
      max_per: 12,
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
