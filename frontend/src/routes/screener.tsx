import { useQueries, useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'

import { ScreenerPageView } from '#components/screener/screener-page-view'
import { useSavedScreens } from '#components/screener/use-saved-screens'
import { $api } from '#lib/api/client'
import type { SavedScreen } from '#lib/screener-search'
import {
  getScreenerRangeError,
  getScreenerSearchAfterSortBy,
  getScreenerSearchAfterSortOrder,
  toScreenerApiQuery,
  toScreenerSearch,
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
  const savedScreens = useSavedScreens({
    search,
    onSelect: (savedScreen) => {
      void navigateToSavedScreen(savedScreen)
    },
    onClearSelection: () => {
      void navigate({
        search: (previous) => ({ ...previous, screen: undefined }),
      })
    },
  })
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
      savedScreens={savedScreens.savedScreens}
      savedScreensPending={savedScreens.listPending}
      savedScreensError={savedScreens.listError}
      savedScreenDialog={savedScreens.dialog}
      savedScreenMutationPending={savedScreens.isPending}
      savedScreenMutationError={savedScreens.error}
      onSelectSavedScreen={(savedScreen) => {
        void navigateToSavedScreen(savedScreen)
      }}
      onSavedScreenDialogChange={savedScreens.onDialogChange}
      onSaveSavedScreen={(name, dialog, filters) => {
        savedScreens.onSave(name, dialog, filters)
      }}
      onOverwriteSavedScreen={savedScreens.onOverwrite}
      onDeleteSavedScreen={savedScreens.onDelete}
    />
  )

  function navigateToSavedScreen(savedScreen: SavedScreen) {
    return navigate({
      search: (previous) => ({
        ...previous,
        ...toScreenerSearch(savedScreen),
        screen: savedScreen.id,
      }),
    })
  }
}
