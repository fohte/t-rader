import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']
type Annotation = components['schemas']['Annotation']
type Stock = components['schemas']['Stock']
type Trade = components['schemas']['TradeListItem']
type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']

export type StockPageListState = 'ready' | 'loading' | 'error'

export type StockLookupResult =
  { kind: 'found'; stock: Stock } | { kind: 'not-found' } | { kind: 'error' }

export function resolveStockLookup({
  status,
  stock,
  error,
}: {
  status: number
  stock: Stock | undefined
  error: unknown
}): StockLookupResult {
  if (status === 404) return { kind: 'not-found' }
  if (error != null || stock == null) return { kind: 'error' }

  return { kind: 'found', stock }
}

export function getStockPageListState({
  isPending,
  isError,
}: {
  isPending: boolean
  isError: boolean
}): StockPageListState {
  if (isPending) return 'loading'
  return isError ? 'error' : 'ready'
}

export interface StockQuote {
  close: number
  change: number | null
  changePercent: number | null
  timestamp: string
}

export function buildStockQuote(bars: Bar[]): StockQuote | null {
  const orderedBars = [...bars].sort((left, right) =>
    left.timestamp.localeCompare(right.timestamp),
  )
  const latest = orderedBars.at(-1)

  if (latest == null) return null

  const previous = orderedBars.at(-2)
  const change = previous == null ? null : latest.close - previous.close

  return {
    close: latest.close,
    change,
    changePercent:
      previous == null || previous.close === 0
        ? null
        : Math.round(
            ((latest.close - previous.close) / previous.close) * 10_000,
          ) / 100,
    timestamp: latest.timestamp,
  }
}

export function sortAnnotationsNewestFirst(
  annotations: Annotation[],
): Annotation[] {
  return [...annotations].sort((left, right) =>
    (right.timestamp_start ?? right.timestamp).localeCompare(
      left.timestamp_start ?? left.timestamp,
    ),
  )
}

export function sortTradesNewestFirst(trades: Trade[]): Trade[] {
  return [...trades].sort((left, right) => right.date.localeCompare(left.date))
}

export function sortFinancialSummariesNewestFirst(
  summaries: FinancialSummary[],
): FinancialSummary[] {
  return [...summaries].sort((left, right) =>
    right.disclosure_date.localeCompare(left.disclosure_date),
  )
}
