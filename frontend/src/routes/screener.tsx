import { useQueries, useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'

import { ScreenerPageView } from '#components/screener/screener-page-view'
import { $api } from '#lib/api/client'
import {
  getScreenerRangeError,
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
  toScreenerApiQuery,
  validateScreenerSearch,
} from '#lib/screener-search'

export const Route = createFileRoute('/screener')({
  validateSearch: validateScreenerSearch,
  component: ScreenerRoute,
})

function ScreenerRoute() {
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const page = search.page ?? 1
  const rangeError = getScreenerRangeError(search)
  const pages = useQueries({
    queries:
      rangeError == null
        ? Array.from({ length: page }, (_, index) =>
            $api.queryOptions('get', '/api/screener', {
              params: { query: toScreenerApiQuery(search, index + 1) },
            }),
          )
        : [],
  })
  const sectorsQuery = useQuery({
    ...$api.queryOptions('get', '/api/screener/sectors'),
    staleTime: 60 * 60 * 1000,
  })

  const firstPage = pages[0]?.data
  const stocks = pages.flatMap((query) => query.data?.items ?? [])
  const isPending = pages.some((query) => query.isPending)
  const isError = pages.some((query) => query.isError)
  const status =
    rangeError != null || isError
      ? 'error'
      : firstPage == null && isPending
        ? 'loading'
        : firstPage == null
          ? 'error'
          : 'results'
  const sectorOptions = sectorsQuery.data
  const errorMessage =
    rangeError ??
    (firstPage == null
      ? '銘柄の取得に失敗しました。もう一度お試しください。'
      : undefined)

  return (
    <ScreenerPageView
      search={search}
      data={firstPage}
      stocks={stocks}
      sectorOptions={sectorOptions ?? []}
      sectorOptionsPending={sectorsQuery.isPending}
      sectorOptionsError={
        sectorsQuery.isError ||
        (sectorsQuery.data == null && !sectorsQuery.isPending)
      }
      status={status}
      errorMessage={errorMessage}
      loadingMore={firstPage != null && isPending}
      onApplyFilters={(filters) => {
        void navigate({
          search: (previous) => ({ ...previous, ...filters, page: undefined }),
        })
      }}
      onSort={(field) => {
        void navigate({
          search: (previous) => getScreenerSearchAfterSortBy(previous, field),
        })
      }}
      onSortOrder={(sortOrder) => {
        void navigate({
          search: (previous) =>
            getScreenerSearchAfterSortOrder(previous, sortOrder),
        })
      }}
      onLoadMore={() => {
        void navigate({
          search: (previous) => ({ ...previous, page: page + 1 }),
        })
      }}
    />
  )
}
