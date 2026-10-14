import { StockPageGroups } from '#components/stock-page/stock-page-groups'
import { formatYen, pnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'
import { getChartCurrency } from '#lib/chart-utils'
import type { StockPageListState, StockQuote } from '#lib/stock-page-model'

type Stock = components['schemas']['Stock']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Asia/Tokyo',
})

const MARKET_LABELS: Record<string, string> = {
  TSE: '東証',
  US: '米国',
}

function formatPrice(value: number, market: string | null | undefined): string {
  const currency = getChartCurrency(market)
  if (currency === 'JPY') return formatYen(value)
  if (currency == null) return value.toLocaleString('ja-JP')

  return new Intl.NumberFormat('ja-JP', {
    style: 'currency',
    currency,
    maximumFractionDigits: 2,
  }).format(value)
}

function formatChange(
  value: number,
  market: string | null | undefined,
): string {
  return `${signOf(value)}${formatPrice(Math.abs(value), market)}`
}

function signOf(value: number): string {
  return value > 0 ? '+' : value < 0 ? '−' : ''
}

export function StockPageHeader({
  stock,
  quote,
  isHolding,
  groups,
  groupsState,
  quoteLoading = false,
  quoteError = false,
}: {
  stock: Stock
  quote: StockQuote | null
  isHolding: boolean
  groups: GroupMembership[]
  groupsState: StockPageListState
  quoteLoading?: boolean
  quoteError?: boolean
}) {
  const change = quote?.change
  const changePercent = quote?.changePercent

  return (
    <header className="flex flex-col gap-3 border-b border-border pb-4 sm:flex-row sm:items-start sm:justify-between">
      <div className="min-w-0">
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h1 className="text-2xl font-bold leading-tight tracking-tight">
            {stock.name}
          </h1>
          <span className="font-mono text-sm text-muted-foreground">
            {stock.id}
          </span>
          {stock.market != null && (
            <span className="font-mono text-xs text-muted-foreground-strong">
              {MARKET_LABELS[stock.market] ?? stock.market}
            </span>
          )}
          {isHolding && (
            <span className="border border-primary/50 px-1.5 py-0.5 font-mono text-2xs text-primary">
              保有
            </span>
          )}
        </div>
        <StockPageGroups
          memberships={groups}
          state={groupsState}
          className="mt-2 hidden sm:flex"
        />
      </div>

      <div className="shrink-0 sm:text-right">
        {quoteLoading ? (
          <p className="font-mono text-sm text-muted-foreground">読み込み中</p>
        ) : quoteError ? (
          <p className="font-mono text-sm text-destructive">
            終値の取得に失敗しました
          </p>
        ) : quote == null ? (
          <p className="font-mono text-sm text-muted-foreground">終値なし</p>
        ) : (
          <>
            <p className="font-mono text-2xl font-bold tabular-nums">
              {formatPrice(quote.close, stock.market)}
            </p>
            <div className="flex flex-wrap items-baseline gap-x-2 font-mono text-xs sm:justify-end">
              {change != null && changePercent != null && (
                <span className={pnlColorClass(change)}>
                  {formatChange(change, stock.market)} ({signOf(change)}
                  {Math.abs(changePercent).toFixed(2)}%)
                </span>
              )}
              <time
                className="text-muted-foreground"
                dateTime={quote.timestamp}
              >
                {dateFormatter.format(new Date(quote.timestamp))} 終値
              </time>
            </div>
          </>
        )}
      </div>
    </header>
  )
}
