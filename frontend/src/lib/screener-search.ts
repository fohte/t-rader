import type { components, operations } from '#lib/api/schema.gen'

export type ScreenerSortBy = components['schemas']['ScreenerSortBy']
export type ScreenerSortOrder = components['schemas']['ScreenerSortOrder']

export const SCREENER_SORT_LABELS: Record<ScreenerSortBy, string> = {
  id: '銘柄コード',
  name: '銘柄',
  market: '市場',
  sector: '業種',
  close: '終値',
  per: 'PER',
  pbr: 'PBR',
  roe: 'ROE',
  dividend_yield: '配当利回り (予)',
  market_cap: '時価総額',
}

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
  screen?: string
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
export type SavedScreen = components['schemas']['SavedScreen']
export type SaveScreenRequest = components['schemas']['SaveScreenRequest']
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
  const screen =
    typeof search.screen === 'string' && search.screen.trim() !== ''
      ? search.screen
      : undefined
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
    screen,
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

export function toSavedScreenRequest(
  name: string,
  search: ScreenerSearch,
): SaveScreenRequest {
  const range = (min?: number, max?: number) =>
    min == null && max == null ? null : { min, max }

  return {
    name: name.trim(),
    filters: {
      markets: [...(search.markets ?? [])].sort(),
      sectors: [...(search.sectors ?? [])].sort(),
      per: range(search.min_per, search.max_per),
      pbr: range(search.min_pbr, search.max_pbr),
      roe: range(search.min_roe, search.max_roe),
      dividend_yield: range(
        search.min_dividend_yield,
        search.max_dividend_yield,
      ),
      market_cap: range(search.min_market_cap, search.max_market_cap),
    },
    sort: {
      field: search.sort_by ?? DEFAULT_SCREENER_SORT.sort_by,
      direction: search.sort_order ?? DEFAULT_SCREENER_SORT.sort_order,
    },
  }
}

export function toScreenerSearch(savedScreen: SavedScreen): ScreenerSearch {
  const { filters, sort } = savedScreen
  return {
    markets:
      filters.markets.length === 0
        ? undefined
        : filters.markets.filter((market): market is ScreenerMarket =>
            SCREENER_MARKETS.some((option) => option === market),
          ),
    sectors: filters.sectors.length === 0 ? undefined : filters.sectors,
    min_per: filters.per?.min ?? undefined,
    max_per: filters.per?.max ?? undefined,
    min_pbr: filters.pbr?.min ?? undefined,
    max_pbr: filters.pbr?.max ?? undefined,
    min_roe: filters.roe?.min ?? undefined,
    max_roe: filters.roe?.max ?? undefined,
    min_dividend_yield: filters.dividend_yield?.min ?? undefined,
    max_dividend_yield: filters.dividend_yield?.max ?? undefined,
    min_market_cap: filters.market_cap?.min ?? undefined,
    max_market_cap: filters.market_cap?.max ?? undefined,
    sort_by: sort.field,
    sort_order: sort.direction,
    page: undefined,
  }
}

export function isSavedScreenSelectionOnly(search: ScreenerSearch): boolean {
  return (
    search.screen != null &&
    (search.markets?.length ?? 0) === 0 &&
    (search.sectors?.length ?? 0) === 0 &&
    SCREENER_RANGE_FIELDS.every(
      ({ minField, maxField }) =>
        search[minField] == null && search[maxField] == null,
    ) &&
    search.sort_by == null &&
    search.sort_order == null
  )
}

export function restoreSavedScreenSearch(
  search: ScreenerSearch,
  savedScreens: SavedScreen[],
): ScreenerSearch | undefined {
  if (!isSavedScreenSelectionOnly(search)) return undefined

  const savedScreen = savedScreens.find(
    (candidate) => candidate.id === search.screen,
  )
  if (savedScreen == null) return undefined

  return { ...toScreenerSearch(savedScreen), screen: savedScreen.id }
}

export function savedScreenHasSearchChanges(
  savedScreen: SavedScreen,
  search: ScreenerSearch,
): boolean {
  const savedConditions = toSavedScreenRequest(
    savedScreen.name,
    toScreenerSearch(savedScreen),
  )
  const currentConditions = toSavedScreenRequest(savedScreen.name, search)

  return (
    JSON.stringify(savedConditions.filters) !==
      JSON.stringify(currentConditions.filters) ||
    JSON.stringify(savedConditions.sort) !==
      JSON.stringify(currentConditions.sort)
  )
}

export function buildScreenerAgentRequest(
  search: ScreenerSearch,
  savedScreen?: SavedScreen,
  sectorOptions: ScreenerSectorOption[] = [],
): string {
  if (
    savedScreen != null &&
    search.screen === savedScreen.id &&
    !savedScreenHasSearchChanges(savedScreen, search)
  ) {
    return `保存した条件「${savedScreen.name}」(screen_id: ${savedScreen.id}) で銘柄候補を探してください。`
  }

  const markets = search.markets ?? []
  const sectors = search.sectors ?? []
  const conditions = [
    ...(markets.length > 0 ? [`市場: ${markets.join('、')}`] : []),
    ...(sectors.length > 0
      ? [
          `業種: ${sectors
            .map(
              (key) =>
                sectorOptions.find((option) => option.key === key)?.name ?? key,
            )
            .join('、')}`,
        ]
      : []),
    ...SCREENER_RANGE_FIELDS.flatMap(({ label, unit, minField, maxField }) => {
      const min = search[minField]
      const max = search[maxField]
      if (min == null && max == null) return []
      let range: string
      if (min == null) {
        range = `${String(max)}以下`
      } else if (max == null) {
        range = `${String(min)}以上`
      } else {
        range = `${String(min)}〜${String(max)}`
      }
      return [`${label}: ${range}${unit}`]
    }),
  ]
  const sortBy = search.sort_by ?? DEFAULT_SCREENER_SORT.sort_by
  const sortOrder = search.sort_order ?? DEFAULT_SCREENER_SORT.sort_order

  return [
    '次の条件と並び順で銘柄候補を探してください。',
    `条件: ${conditions.length === 0 ? 'なし (全銘柄)' : conditions.join('、')}`,
    `並び順: ${SCREENER_SORT_LABELS[sortBy]} (${sortOrder === 'asc' ? '昇順' : '降順'})`,
  ].join('\n')
}

export function clearSavedScreenSelectionSearch(
  search: ScreenerSearch,
): ScreenerSearch {
  return { ...search, screen: undefined }
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

function getNextScreenerSort(
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
