import type { components, operations } from '#lib/api/schema.gen'

export type ScreenerSortBy = components['schemas']['ScreenerSortBy']
export type ScreenerSortOrder = components['schemas']['ScreenerSortOrder']

export const SCREENER_MARKETS = [
  'プライム',
  'スタンダード',
  'グロース',
] as const
export type ScreenerMarket = (typeof SCREENER_MARKETS)[number]

export type ScreenerSearch = {
  markets?: ScreenerMarket[]
  sectors?: string[]
  min_per?: number
  max_per?: number
  min_pbr?: number
  max_pbr?: number
  min_roe?: number
  max_roe?: number
  min_dividend_yield?: number
  max_dividend_yield?: number
  min_market_cap?: number
  max_market_cap?: number
  sort_by?: ScreenerSortBy
  sort_order?: ScreenerSortOrder
  page?: number
}

export type ScreenerSectorOption =
  components['schemas']['ScreenerSectorResponse']
export type ScreenerApiQuery = NonNullable<
  operations['screen_stocks']['parameters']['query']
>

const SORT_FIELDS: readonly ScreenerSortBy[] = [
  'id',
  'name',
  'market',
  'sector',
  'close',
  'per',
  'pbr',
  'roe',
  'dividend_yield',
  'market_cap',
]

const RANGE_FIELDS = [
  ['min_per', 'max_per'],
  ['min_pbr', 'max_pbr'],
  ['min_roe', 'max_roe'],
  ['min_dividend_yield', 'max_dividend_yield'],
  ['min_market_cap', 'max_market_cap'],
] as const
const MAX_SCREENER_PAGE = 100

function parseList(value: unknown): string[] | undefined {
  const values = Array.isArray(value) ? value : [value]
  const items = values
    .flatMap((item) => (typeof item === 'string' ? item.split(',') : []))
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
  return items.length === 0 ? undefined : [...new Set(items)]
}

function parseNumber(value: unknown): number | undefined {
  if (typeof value !== 'number' && typeof value !== 'string') return undefined
  if (typeof value === 'string' && value.trim() === '') return undefined
  const number = Number(value)
  return Number.isFinite(number) ? number : undefined
}

function parsePage(value: unknown): number | undefined {
  const page = parseNumber(value)
  if (page == null || !Number.isInteger(page) || page < 1) return undefined
  return Math.min(page, MAX_SCREENER_PAGE)
}

export function validateScreenerSearch(
  search: Record<string, unknown>,
): ScreenerSearch {
  const requestedMarkets = parseList(search.markets)
  const markets = requestedMarkets?.filter((market): market is ScreenerMarket =>
    SCREENER_MARKETS.some((option) => option === market),
  )
  const sectors = parseList(search.sectors)
  const sortBy = SORT_FIELDS.find((field) => field === search.sort_by)

  return {
    markets: markets == null || markets.length === 0 ? undefined : markets,
    sectors,
    min_per: parseNumber(search.min_per),
    max_per: parseNumber(search.max_per),
    min_pbr: parseNumber(search.min_pbr),
    max_pbr: parseNumber(search.max_pbr),
    min_roe: parseNumber(search.min_roe),
    max_roe: parseNumber(search.max_roe),
    min_dividend_yield: parseNumber(search.min_dividend_yield),
    max_dividend_yield: parseNumber(search.max_dividend_yield),
    min_market_cap: parseNumber(search.min_market_cap),
    max_market_cap: parseNumber(search.max_market_cap),
    sort_by: sortBy,
    sort_order:
      search.sort_order === 'asc' || search.sort_order === 'desc'
        ? search.sort_order
        : undefined,
    page: parsePage(search.page),
  }
}

export function getScreenerRangeError(
  search: Pick<ScreenerSearch, (typeof RANGE_FIELDS)[number][number]>,
): string | undefined {
  return RANGE_FIELDS.some(([minField, maxField]) => {
    const min = search[minField]
    const max = search[maxField]
    return min != null && max != null && min > max
  })
    ? '下限は上限以下にしてください。'
    : undefined
}

export function countActiveScreenerFilters(search: ScreenerSearch): number {
  return [
    (search.markets?.length ?? 0) > 0,
    (search.sectors?.length ?? 0) > 0,
    search.min_per != null || search.max_per != null,
    search.min_pbr != null || search.max_pbr != null,
    search.min_roe != null || search.max_roe != null,
    search.min_dividend_yield != null || search.max_dividend_yield != null,
    search.min_market_cap != null || search.max_market_cap != null,
  ].filter(Boolean).length
}

export function getNextScreenerSort(
  current: Pick<ScreenerSearch, 'sort_by' | 'sort_order'>,
  field: ScreenerSortBy,
): Pick<ScreenerSearch, 'sort_by' | 'sort_order'> {
  const currentField = current.sort_by ?? 'market_cap'
  const currentOrder = current.sort_order ?? 'desc'
  if (field === currentField) {
    return {
      sort_by: field,
      sort_order: currentOrder === 'asc' ? 'desc' : 'asc',
    }
  }

  return {
    sort_by: field,
    sort_order:
      field === 'id' ||
      field === 'name' ||
      field === 'market' ||
      field === 'sector'
        ? 'asc'
        : 'desc',
  }
}

export function getScreenerSearchAfterSortBy(
  current: ScreenerSearch,
  field: ScreenerSortBy,
): ScreenerSearch {
  return {
    ...current,
    ...getNextScreenerSort(current, field),
    page: undefined,
  }
}

export function getScreenerSearchAfterSortOrder(
  current: ScreenerSearch,
  order: ScreenerSortOrder,
): ScreenerSearch {
  return { ...current, sort_order: order, page: undefined }
}

export function toScreenerApiQuery(
  search: ScreenerSearch,
  page: number,
): ScreenerApiQuery {
  return {
    markets: search.markets?.join(','),
    sectors: search.sectors?.join(','),
    min_per: search.min_per,
    max_per: search.max_per,
    min_pbr: search.min_pbr,
    max_pbr: search.max_pbr,
    min_roe: search.min_roe,
    max_roe: search.max_roe,
    min_dividend_yield: search.min_dividend_yield,
    max_dividend_yield: search.max_dividend_yield,
    min_market_cap: search.min_market_cap,
    max_market_cap: search.max_market_cap,
    sort_by: search.sort_by,
    sort_order: search.sort_order,
    page,
  }
}
