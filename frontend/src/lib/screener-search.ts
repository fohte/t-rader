import type { components, operations } from '#lib/api/schema.gen'

export type ScreenerSortBy = components['schemas']['ScreenerSortBy']
export type ScreenerSortOrder = components['schemas']['ScreenerSortOrder']

export const SCREENER_MARKETS = [
  'プライム',
  'スタンダード',
  'グロース',
] as const
export type ScreenerMarket = (typeof SCREENER_MARKETS)[number]
export const DEFAULT_SCREENER_SORT = {
  sort_by: 'market_cap',
  sort_order: 'desc',
} as const

export const SCREENER_RANGE_FIELDS = [
  {
    key: 'per',
    label: 'PER',
    unit: '倍',
    minField: 'min_per',
    maxField: 'max_per',
  },
  {
    key: 'pbr',
    label: 'PBR',
    unit: '倍',
    minField: 'min_pbr',
    maxField: 'max_pbr',
  },
  {
    key: 'roe',
    label: 'ROE',
    unit: '%',
    minField: 'min_roe',
    maxField: 'max_roe',
  },
  {
    key: 'dividend_yield',
    label: '配当利回り (予)',
    unit: '%',
    minField: 'min_dividend_yield',
    maxField: 'max_dividend_yield',
  },
  {
    key: 'market_cap',
    label: '時価総額',
    unit: '億円',
    minField: 'min_market_cap',
    maxField: 'max_market_cap',
  },
] as const
export type ScreenerRangeSearchKey =
  | (typeof SCREENER_RANGE_FIELDS)[number]['minField']
  | (typeof SCREENER_RANGE_FIELDS)[number]['maxField']

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

const MAX_SCREENER_PAGE = 100

export function isScreenerSortBy(value: string): value is ScreenerSortBy {
  return SORT_FIELDS.some((field) => field === value)
}

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
  const sortBy =
    typeof search.sort_by === 'string' && isScreenerSortBy(search.sort_by)
      ? search.sort_by
      : undefined

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
  search: Pick<ScreenerSearch, ScreenerRangeSearchKey>,
): string | undefined {
  return SCREENER_RANGE_FIELDS.some(({ minField, maxField }) => {
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
    ...SCREENER_RANGE_FIELDS.map(
      ({ minField, maxField }) =>
        search[minField] != null || search[maxField] != null,
    ),
  ].filter(Boolean).length
}

export function getNextScreenerSort(
  current: Pick<ScreenerSearch, 'sort_by' | 'sort_order'>,
  field: ScreenerSortBy,
): Pick<ScreenerSearch, 'sort_by' | 'sort_order'> {
  const currentField = current.sort_by ?? DEFAULT_SCREENER_SORT.sort_by
  const currentOrder = current.sort_order ?? DEFAULT_SCREENER_SORT.sort_order
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
