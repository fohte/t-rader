import { Link } from '@tanstack/react-router'
import { ChevronDown, ChevronUp } from 'lucide-react'

import { formatScreenerMetric } from '#components/screener/format'
import { ScreenerStockCard } from '#components/screener/screener-stock-card'
import { formatYen } from '#components/trades/format'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import type {
  ScreenerSearch,
  ScreenerSortBy,
  ScreenerSortOrder,
} from '#lib/screener-search'
import {
  DEFAULT_SCREENER_SORT,
  isScreenerSortBy,
  SCREENER_SORT_LABELS,
} from '#lib/screener-search'

type ScreenerStock = components['schemas']['ScreenerStockResponse']
type ScreenerResponse = components['schemas']['ScreenerResponse']
export type ScreenerResultsStatus = 'loading' | 'error' | 'results'

const SORTABLE_COLUMNS: {
  key: ScreenerSortBy
  label: string
  align: 'left' | 'right'
}[] = [
  { key: 'name', label: SCREENER_SORT_LABELS.name, align: 'left' },
  { key: 'market', label: SCREENER_SORT_LABELS.market, align: 'left' },
  { key: 'sector', label: SCREENER_SORT_LABELS.sector, align: 'left' },
  { key: 'close', label: SCREENER_SORT_LABELS.close, align: 'right' },
  { key: 'per', label: SCREENER_SORT_LABELS.per, align: 'right' },
  { key: 'pbr', label: SCREENER_SORT_LABELS.pbr, align: 'right' },
  { key: 'roe', label: SCREENER_SORT_LABELS.roe, align: 'right' },
  {
    key: 'dividend_yield',
    label: SCREENER_SORT_LABELS.dividend_yield,
    align: 'right',
  },
  { key: 'market_cap', label: SCREENER_SORT_LABELS.market_cap, align: 'right' },
]
const MOBILE_SORT_OPTIONS: { key: ScreenerSortBy; label: string }[] = [
  { key: 'id', label: SCREENER_SORT_LABELS.id },
  ...SORTABLE_COLUMNS.map(({ key, label }) => ({ key, label })),
]

function formatAsOf(value: string | null | undefined): string {
  if (value == null) return '—'
  const [, , month, day] = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value) ?? []
  const displayMonth = Number(month).toString()
  const displayDay = Number(day).toString()
  return month == null || day == null ? value : `${displayMonth}/${displayDay}`
}

function isScreenerSortOrder(value: string): value is ScreenerSortOrder {
  return value === 'asc' || value === 'desc'
}

function isSortedColumn(
  sortField: ScreenerSortBy,
  columnKey: ScreenerSortBy,
): boolean {
  return sortField === columnKey || (columnKey === 'name' && sortField === 'id')
}

function getStockValue(stock: ScreenerStock, key: ScreenerSortBy): string {
  switch (key) {
    case 'name':
    case 'id':
      return stock.name
    case 'market':
      return stock.market ?? '—'
    case 'sector':
      return stock.sector ?? '—'
    case 'close':
      return stock.close == null ? '—' : formatYen(stock.close)
    case 'per':
      return formatScreenerMetric(stock.per, '倍')
    case 'pbr':
      return formatScreenerMetric(stock.pbr, '倍')
    case 'roe':
      return formatScreenerMetric(stock.roe, '%')
    case 'dividend_yield':
      return formatScreenerMetric(stock.dividend_yield, '%')
    case 'market_cap':
      return formatScreenerMetric(stock.market_cap, '億円')
  }
}

export function ScreenerResultsView({
  search,
  data,
  stocks,
  status,
  errorMessage,
  loadingMore = false,
  loadMoreError = false,
  onSort,
  onSortOrder,
  onLoadMore,
  onRetryLoadMore,
  onAskAgent,
}: {
  search: Pick<ScreenerSearch, 'sort_by' | 'sort_order'>
  data: ScreenerResponse | undefined
  stocks: ScreenerStock[]
  status: ScreenerResultsStatus
  errorMessage?: string
  loadingMore?: boolean
  loadMoreError?: boolean
  onSort: (field: ScreenerSortBy) => void
  onSortOrder: (order: ScreenerSortOrder) => void
  onLoadMore: () => void
  onRetryLoadMore: () => void
  onAskAgent?: () => void
}) {
  const sortField = search.sort_by ?? DEFAULT_SCREENER_SORT.sort_by
  const sortOrder = search.sort_order ?? DEFAULT_SCREENER_SORT.sort_order
  const hasMore = data != null && stocks.length < data.total_count
  const nextCount =
    data == null ? 0 : Math.min(100, data.total_count - stocks.length)

  return (
    <div className="min-w-0">
      <div
        aria-live="polite"
        className="mb-3 flex min-h-9 flex-wrap items-center justify-between gap-2 border-y border-border bg-card px-3 py-2"
      >
        <span className="font-semibold">
          {status === 'results' && data != null
            ? `該当 ${data.total_count.toLocaleString('ja-JP')} 銘柄`
            : status === 'loading'
              ? '銘柄を検索しています...'
              : '検索結果'}
        </span>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {status === 'results' && data != null && (
            <span className="text-muted-foreground">
              {formatAsOf(data.as_of)} 時点
            </span>
          )}
          {onAskAgent != null && (
            <button
              type="button"
              onClick={() => {
                onAskAgent()
              }}
              className="hidden items-center gap-1 border border-primary bg-primary px-3 py-2 font-semibold text-primary-foreground hover:opacity-90 md:inline-flex"
            >
              <span aria-hidden="true" className="font-bold">
                &gt;_
              </span>
              この条件でエージェントに頼む
            </button>
          )}
        </div>
      </div>

      {status === 'results' && (
        <div className="mb-3 flex items-center gap-2 md:hidden">
          <span className="shrink-0 text-muted-foreground">並び:</span>
          <label className="sr-only" htmlFor="screener-mobile-sort-field">
            並べ替える列
          </label>
          <select
            id="screener-mobile-sort-field"
            aria-label="並べ替える列"
            value={sortField}
            onChange={(event) => {
              const field = event.currentTarget.value
              if (isScreenerSortBy(field)) onSort(field)
            }}
            className="min-w-0 flex-1 border border-border bg-card px-2 py-1.5 text-2xs text-foreground"
          >
            {MOBILE_SORT_OPTIONS.map((option) => (
              <option key={option.key} value={option.key}>
                {option.label}
              </option>
            ))}
          </select>
          <label className="sr-only" htmlFor="screener-mobile-sort-order">
            並べ替え順
          </label>
          <select
            id="screener-mobile-sort-order"
            aria-label="並べ替え順"
            value={sortOrder}
            onChange={(event) => {
              const order = event.currentTarget.value
              if (isScreenerSortOrder(order)) onSortOrder(order)
            }}
            className="shrink-0 border border-border bg-card px-2 py-1.5 text-2xs text-foreground"
          >
            <option value="desc">降順</option>
            <option value="asc">昇順</option>
          </select>
        </div>
      )}

      {status === 'loading' ? (
        <div className="space-y-2" aria-label="銘柄を読み込み中">
          {[0, 1, 2, 3].map((row) => (
            <Skeleton key={row} className="h-12 w-full rounded-none" />
          ))}
        </div>
      ) : status === 'error' ? (
        <div
          role="alert"
          className="border border-destructive/50 bg-card px-4 py-10 text-center text-destructive"
        >
          {errorMessage ?? '銘柄の取得に失敗しました。'}
        </div>
      ) : stocks.length === 0 ? (
        <div className="border border-border bg-card px-4 py-10 text-center">
          <p className="font-semibold">該当する銘柄はありません。</p>
          <p className="mt-2 text-muted-foreground">
            条件を変更して、もう一度検索してください。
          </p>
        </div>
      ) : (
        <>
          <div className="hidden overflow-x-auto border border-border bg-card md:block">
            <table className="w-full table-fixed font-mono text-2xs md:text-xs">
              <colgroup>
                <col className="w-1/6" />
                <col className="w-1/12" />
                <col />
                <col />
                <col />
                <col />
                <col />
                <col />
                <col />
              </colgroup>
              <thead>
                <tr className="border-b border-border text-3xs tracking-wide text-muted-foreground md:text-2xs">
                  {SORTABLE_COLUMNS.map((column) => {
                    const isSorted = isSortedColumn(sortField, column.key)
                    return (
                      <th
                        key={column.key}
                        scope="col"
                        aria-sort={
                          isSorted
                            ? sortOrder === 'asc'
                              ? 'ascending'
                              : 'descending'
                            : 'none'
                        }
                        className={`whitespace-nowrap px-1.5 py-2 font-normal md:px-2.5 ${column.align === 'right' ? 'text-right' : 'text-left'}`}
                      >
                        <button
                          type="button"
                          onClick={() => {
                            onSort(column.key)
                          }}
                          className="inline-flex items-center gap-1 hover:text-foreground"
                          aria-label={`${column.label} で並べ替え`}
                        >
                          {column.label}
                          {isSorted &&
                            (sortOrder === 'asc' ? (
                              <ChevronUp
                                aria-hidden="true"
                                className="size-3"
                              />
                            ) : (
                              <ChevronDown
                                aria-hidden="true"
                                className="size-3"
                              />
                            ))}
                        </button>
                      </th>
                    )
                  })}
                </tr>
              </thead>
              <tbody>
                {stocks.map((stock) => (
                  <tr
                    key={stock.id}
                    className="border-b border-border last:border-b-0 hover:bg-surface-strong"
                  >
                    <td className="overflow-hidden px-1.5 py-2 md:px-2.5">
                      <Link
                        to="/stocks/$stockId"
                        params={{ stockId: stock.id }}
                        className="block overflow-hidden hover:text-primary hover:underline"
                      >
                        <span className="block truncate font-semibold">
                          {stock.name}
                        </span>
                        <span className="text-2xs text-muted-foreground">
                          {stock.id}
                        </span>
                      </Link>
                    </td>
                    {SORTABLE_COLUMNS.slice(1).map((column) => (
                      <td
                        key={column.key}
                        className={`overflow-hidden px-1.5 py-2 md:px-2.5 ${column.align === 'right' ? 'text-right tabular-nums' : 'truncate text-left'}`}
                      >
                        {getStockValue(stock, column.key)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <ul className="space-y-2 md:hidden">
            {stocks.map((stock) => (
              <li key={stock.id}>
                <ScreenerStockCard stock={stock} />
              </li>
            ))}
          </ul>

          {hasMore &&
            (loadMoreError ? (
              <div className="mt-3 border border-destructive/50 bg-card px-3 py-3 text-center">
                <p role="alert" className="text-destructive">
                  追加の銘柄を取得できませんでした。
                </p>
                <button
                  type="button"
                  onClick={onRetryLoadMore}
                  disabled={loadingMore}
                  className="mt-2 border border-border px-3 py-1.5 font-semibold text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground disabled:cursor-wait disabled:opacity-60"
                >
                  {loadingMore ? '追加の銘柄を読み込んでいます...' : '再試行'}
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={onLoadMore}
                disabled={loadingMore}
                className="mt-3 w-full border border-border bg-card px-3 py-2.5 font-semibold text-muted-foreground-strong hover:border-muted-foreground hover:bg-surface-strong hover:text-foreground disabled:cursor-wait disabled:opacity-60"
              >
                {loadingMore
                  ? '追加の銘柄を読み込んでいます...'
                  : `さらに ${nextCount.toLocaleString('ja-JP')} 件を表示`}
              </button>
            ))}
        </>
      )}
    </div>
  )
}
