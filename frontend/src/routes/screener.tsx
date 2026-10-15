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
  const page = Number(search.page ?? 1)
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

  const firstPageQuery = pages[0]
  const firstPage = firstPageQuery?.data
  const stocks = pages.flatMap((query) => query.data?.items ?? [])
  const failedLaterQuery = pages
    .slice(1)
    .find((query) => query.isError && query.data == null)
  const status =
    rangeError != null
      ? 'error'
      : firstPage != null
        ? 'results'
        : firstPageQuery?.isPending === true
          ? 'loading'
          : 'error'
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
      loadingMore={
        firstPage != null && pages.slice(1).some((query) => query.isFetching)
      }
      loadMoreError={failedLaterQuery != null}
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
      onRetryLoadMore={() => {
        if (failedLaterQuery != null) void failedLaterQuery.refetch()
      }}
    />
  )
}
