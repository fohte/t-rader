import { Input } from '@fohte/ui/input'
import { ChevronDown, SlidersHorizontal, X } from 'lucide-react'
import { type SyntheticEvent, useEffect, useRef, useState } from 'react'

import {
  type SaveDialogState,
  type SavedScreenDialogState,
  SavedScreensControlView,
} from '#components/screener/saved-screens-control-view'
import {
  type ScreenerResultsStatus,
  ScreenerResultsView,
} from '#components/screener/screener-results-view'
import type { components } from '#lib/api/schema.gen'
import {
  countActiveScreenerFilters,
  getScreenerRangeError,
  type SavedScreen,
  savedScreenHasSearchChanges,
  SCREENER_MARKETS,
  SCREENER_RANGE_FIELDS,
  type ScreenerMarket,
  type ScreenerRangeSearchKey,
  type ScreenerSearch,
  type ScreenerSectorOption,
  type ScreenerSortBy,
  type ScreenerSortOrder,
} from '#lib/screener-search'

type ScreenerStock = components['schemas']['ScreenerStockResponse']
type ScreenerResponse = components['schemas']['ScreenerResponse']

type FilterDraft = Record<ScreenerRangeSearchKey, string> & {
  markets: ScreenerMarket[]
  sectors: string[]
}

function createDraft(search: ScreenerSearch): FilterDraft {
  return {
    markets: search.markets ?? [],
    sectors: search.sectors ?? [],
    min_per: search.min_per?.toString() ?? '',
    max_per: search.max_per?.toString() ?? '',
    min_pbr: search.min_pbr?.toString() ?? '',
    max_pbr: search.max_pbr?.toString() ?? '',
    min_roe: search.min_roe?.toString() ?? '',
    max_roe: search.max_roe?.toString() ?? '',
    min_dividend_yield: search.min_dividend_yield?.toString() ?? '',
    max_dividend_yield: search.max_dividend_yield?.toString() ?? '',
    min_market_cap: search.min_market_cap?.toString() ?? '',
    max_market_cap: search.max_market_cap?.toString() ?? '',
  }
}

function readDraft(draft: FilterDraft): ScreenerSearch {
  const parse = (value: string) => {
    if (value.trim() === '') return undefined
    const number = Number(value)
    return Number.isFinite(number) ? number : undefined
  }
  return {
    markets: draft.markets.length === 0 ? undefined : draft.markets,
    sectors: draft.sectors.length === 0 ? undefined : draft.sectors,
    min_per: parse(draft.min_per),
    max_per: parse(draft.max_per),
    min_pbr: parse(draft.min_pbr),
    max_pbr: parse(draft.max_pbr),
    min_roe: parse(draft.min_roe),
    max_roe: parse(draft.max_roe),
    min_dividend_yield: parse(draft.min_dividend_yield),
    max_dividend_yield: parse(draft.max_dividend_yield),
    min_market_cap: parse(draft.min_market_cap),
    max_market_cap: parse(draft.max_market_cap),
  }
}

export function ScreenerPageView({
  search,
  data,
  stocks,
  sectorOptions,
  sectorOptionsPending,
  sectorOptionsError,
  status,
  errorMessage,
  loadingMore = false,
  loadMoreError = false,
  mobileFiltersOpen: mobileFiltersInitiallyOpen = false,
  savedScreens = [],
  savedScreensPending = false,
  savedScreensError = false,
  savedScreensMenuOpen: savedScreensMenuInitiallyOpen = false,
  savedScreenDialog = null,
  savedScreenMutationPending = false,
  savedScreenMutationError,
  onApplyFilters,
  onSort,
  onSortOrder,
  onLoadMore,
  onRetryLoadMore,
  onAskAgent,
  onSelectSavedScreen = () => {},
  onSavedScreenDialogChange = () => {},
  onSaveSavedScreen = () => {},
  onOverwriteSavedScreen = () => {},
  onDeleteSavedScreen = () => {},
}: {
  search: ScreenerSearch
  data: ScreenerResponse | undefined
  stocks: ScreenerStock[]
  sectorOptions: ScreenerSectorOption[]
  sectorOptionsPending: boolean
  sectorOptionsError: boolean
  status: ScreenerResultsStatus
  errorMessage?: string
  loadingMore?: boolean
  loadMoreError?: boolean
  mobileFiltersOpen?: boolean
  savedScreens?: SavedScreen[]
  savedScreensPending?: boolean
  savedScreensError?: boolean
  savedScreensMenuOpen?: boolean
  savedScreenDialog?: SavedScreenDialogState | null
  savedScreenMutationPending?: boolean
  savedScreenMutationError?: string
  onApplyFilters: (filters: ScreenerSearch) => void
  onSort: (field: ScreenerSortBy) => void
  onSortOrder: (order: ScreenerSortOrder) => void
  onLoadMore: () => void
  onRetryLoadMore: () => void
  onAskAgent?: (search: ScreenerSearch) => void
  onSelectSavedScreen?: (savedScreen: SavedScreen) => void
  onSavedScreenDialogChange?: (dialog: SavedScreenDialogState | null) => void
  onSaveSavedScreen?: (
    name: string,
    dialog: SaveDialogState,
    filters: ScreenerSearch,
  ) => void
  onOverwriteSavedScreen?: (filters: ScreenerSearch) => void
  onDeleteSavedScreen?: (savedScreen: SavedScreen) => void
}) {
  const [draft, setDraft] = useState(() => createDraft(search))
  const [validationError, setValidationError] = useState<string>()
  const [mobileFiltersOpen, setMobileFiltersOpen] = useState(
    mobileFiltersInitiallyOpen,
  )
  const [savedScreensMenuOpen, setSavedScreensMenuOpen] = useState(
    savedScreensMenuInitiallyOpen,
  )
  const conditionCount = countActiveScreenerFilters(search)
  const selectedSavedScreen = savedScreens.find(
    (savedScreen) => savedScreen.id === search.screen,
  )
  const selectedScreenHasChanges =
    selectedSavedScreen != null &&
    savedScreenHasSearchChanges(selectedSavedScreen, search)
  const appliedFilterKey = JSON.stringify(createDraft(search))
  const previousAppliedFilterKey = useRef(appliedFilterKey)

  useEffect(() => {
    if (previousAppliedFilterKey.current === appliedFilterKey) return
    previousAppliedFilterKey.current = appliedFilterKey
    setDraft(createDraft(search))
    setValidationError(undefined)
  }, [appliedFilterKey, search])

  function toggleMarket(value: ScreenerMarket) {
    setDraft((previous) => ({
      ...previous,
      markets: previous.markets.includes(value)
        ? previous.markets.filter((item) => item !== value)
        : [...previous.markets, value],
    }))
  }

  function toggleSector(value: string) {
    setDraft((previous) => ({
      ...previous,
      sectors: previous.sectors.includes(value)
        ? previous.sectors.filter((item) => item !== value)
        : [...previous.sectors, value],
    }))
  }

  function renderRangeInput(
    field: ScreenerRangeSearchKey,
    label: string,
    unit: string,
    bound: 'min' | 'max',
  ) {
    return (
      <div key={bound} className="relative min-w-0 flex-1">
        <Input
          aria-label={`${label} ${bound === 'min' ? '下限' : '上限'}`}
          type="number"
          step="any"
          placeholder={bound === 'min' ? '下限なし' : '上限なし'}
          value={draft[field]}
          onChange={(event) => {
            const value = event.currentTarget.value
            setDraft((previous) => ({
              ...previous,
              [field]: value,
            }))
          }}
          className="h-8 pr-8 text-right font-mono text-xs"
        />
        <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-2xs text-muted-foreground">
          {unit}
        </span>
      </div>
    )
  }

  function applyDraft(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault()
    const filters = readDraft(draft)
    const error = getScreenerRangeError(filters)
    setValidationError(error)
    if (error != null) return
    onApplyFilters(filters)
    setMobileFiltersOpen(false)
  }

  function renderFilterFields(
    layout: 'desktop' | 'mobile',
    includeApplyAction = true,
  ) {
    const isDesktop = layout === 'desktop'
    return (
      <>
        <div
          className={
            isDesktop
              ? 'grid grid-cols-1 gap-x-4 gap-y-4 sm:grid-cols-2 lg:grid-cols-4'
              : 'space-y-5'
          }
        >
          <fieldset className={isDesktop ? 'min-w-0 space-y-2' : 'space-y-2'}>
            <legend className="font-semibold text-foreground">市場</legend>
            <div
              className={
                isDesktop
                  ? 'grid grid-cols-2 gap-0.5'
                  : 'grid grid-cols-3 gap-2'
              }
            >
              {SCREENER_MARKETS.map((market) => (
                <label
                  key={market}
                  className={`flex cursor-pointer items-center border border-border bg-background text-muted-foreground-strong has-checked:border-primary has-checked:text-foreground ${isDesktop ? 'gap-1 whitespace-nowrap px-1 py-1.5 text-3xs' : 'gap-1.5 px-2 py-2 text-2xs'}`}
                >
                  <input
                    type="checkbox"
                    checked={draft.markets.includes(market)}
                    onChange={() => {
                      toggleMarket(market)
                    }}
                    className="accent-primary"
                  />
                  {market}
                </label>
              ))}
            </div>
          </fieldset>

          <fieldset className={isDesktop ? 'min-w-0 space-y-2' : 'space-y-2'}>
            <legend className="font-semibold text-foreground">業種</legend>
            <div
              className={`space-y-1 overflow-y-auto border border-border bg-background p-2 ${isDesktop ? 'max-h-36' : 'max-h-44'}`}
            >
              {sectorOptionsPending ? (
                <p className="text-muted-foreground">業種を読み込み中...</p>
              ) : sectorOptionsError ? (
                <p role="alert" className="text-destructive">
                  業種の取得に失敗しました。
                </p>
              ) : sectorOptions.length === 0 ? (
                <p className="text-muted-foreground">
                  業種の選択肢がありません。
                </p>
              ) : (
                sectorOptions.map((sector) => (
                  <label
                    key={sector.key}
                    className="flex cursor-pointer items-center gap-2 py-1 text-muted-foreground-strong hover:text-foreground"
                  >
                    <input
                      type="checkbox"
                      checked={draft.sectors.includes(sector.key)}
                      onChange={() => {
                        toggleSector(sector.key)
                      }}
                      className="accent-primary"
                    />
                    <span>{sector.name}</span>
                  </label>
                ))
              )}
            </div>
          </fieldset>

          {SCREENER_RANGE_FIELDS.map(
            ({ key, label, unit, minField, maxField }) => (
              <fieldset
                key={key}
                className={isDesktop ? 'min-w-0 space-y-2' : 'space-y-2'}
              >
                <legend className="font-semibold text-foreground">
                  {label}
                </legend>
                <div className="flex items-center gap-2">
                  {renderRangeInput(minField, label, unit, 'min')}
                  <span className="text-center text-muted-foreground">–</span>
                  {renderRangeInput(maxField, label, unit, 'max')}
                </div>
              </fieldset>
            ),
          )}
        </div>
        {includeApplyAction && (
          <div
            className={
              isDesktop
                ? 'mt-4 flex flex-col gap-2 border-t border-border pt-3 sm:flex-row sm:items-center sm:justify-between'
                : 'mt-5 space-y-2'
            }
          >
            {validationError != null && (
              <p role="alert" className="text-2xs text-destructive">
                {validationError}
              </p>
            )}
            <button
              type="submit"
              className={`border border-primary bg-primary px-3 py-2 font-semibold text-primary-foreground hover:opacity-90 ${isDesktop ? 'w-full sm:w-auto' : 'w-full'}`}
            >
              この条件で絞り込む
            </button>
          </div>
        )}
      </>
    )
  }

  return (
    <section className="font-mono text-xs text-foreground">
      <header className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-bold tracking-tight md:text-2xl">
            <span className="mr-2 text-primary">&gt;</span>銘柄スクリーナー
          </h1>
          <p className="mt-1 text-2xs text-muted-foreground">
            条件を組み合わせて銘柄を探す
          </p>
        </div>
      </header>

      <form
        onSubmit={applyDraft}
        className="mb-4 border-0 bg-transparent p-0 md:border md:border-border md:bg-card md:p-4"
      >
        <div className="mb-3 flex items-center justify-between gap-2 border-b border-border pb-2 md:mb-3">
          <h2 className="hidden font-semibold tracking-wide md:block">
            <span className="mr-1 text-primary">&gt;</span>条件
          </h2>
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2 md:flex-none md:justify-end">
            <SavedScreensControlView
              savedScreens={savedScreens}
              selectedScreen={selectedSavedScreen}
              selectedScreenId={search.screen}
              selectedScreenHasChanges={selectedScreenHasChanges}
              menuOpen={savedScreensMenuOpen}
              dialog={savedScreenDialog}
              isPending={savedScreenMutationPending}
              error={savedScreenMutationError}
              savedScreensPending={savedScreensPending}
              listError={savedScreensError}
              onMenuOpenChange={setSavedScreensMenuOpen}
              onDialogChange={onSavedScreenDialogChange}
              onSelect={onSelectSavedScreen}
              onSave={(name, dialog) => {
                onSaveSavedScreen(name, dialog, readDraft(draft))
              }}
              onOverwrite={() => {
                onOverwriteSavedScreen(readDraft(draft))
              }}
              onDelete={onDeleteSavedScreen}
            />
            <button
              type="button"
              onClick={() => {
                setMobileFiltersOpen(true)
              }}
              className="inline-flex shrink-0 items-center gap-2 border border-border bg-card px-3 py-2 text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground md:hidden"
            >
              <SlidersHorizontal aria-hidden="true" className="size-3.5" />
              条件 ({conditionCount})
              <ChevronDown aria-hidden="true" className="size-3.5" />
            </button>
          </div>
        </div>
        <div className="hidden md:block">{renderFilterFields('desktop')}</div>
      </form>
      <ScreenerResultsView
        search={search}
        data={data}
        stocks={stocks}
        status={status}
        errorMessage={errorMessage}
        loadingMore={loadingMore}
        loadMoreError={loadMoreError}
        onSort={onSort}
        onSortOrder={onSortOrder}
        onLoadMore={onLoadMore}
        onRetryLoadMore={onRetryLoadMore}
        onAskAgent={
          onAskAgent == null
            ? undefined
            : () => {
                onAskAgent(search)
              }
        }
      />

      {mobileFiltersOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-background md:bg-black/70">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="screener-mobile-filter-title"
            className="flex h-full w-full flex-col bg-background p-4 md:h-full md:w-96 md:border md:border-border"
          >
            <header className="mb-4 flex items-center justify-between border-b border-border pb-3">
              <h2
                id="screener-mobile-filter-title"
                className="font-semibold tracking-wide"
              >
                <span className="mr-1 text-primary">&gt;</span>条件 (
                {conditionCount})
              </h2>
              <button
                type="button"
                onClick={() => {
                  setMobileFiltersOpen(false)
                }}
                aria-label="条件を閉じる"
                className="inline-flex size-8 items-center justify-center border border-border text-muted-foreground-strong hover:text-foreground"
              >
                <X aria-hidden="true" className="size-4" />
              </button>
            </header>
            <form
              onSubmit={applyDraft}
              className="flex min-h-0 flex-1 flex-col"
            >
              <div className="min-h-0 flex-1 overflow-y-auto pb-4">
                {renderFilterFields('mobile', false)}
              </div>
              <div
                role="group"
                aria-label="条件の操作"
                className="mt-4 shrink-0 space-y-2 border-t border-border pt-4"
              >
                {validationError != null && (
                  <p role="alert" className="text-2xs text-destructive">
                    {validationError}
                  </p>
                )}
                <button
                  type="submit"
                  className="w-full border border-primary bg-primary px-3 py-3 font-semibold text-primary-foreground hover:opacity-90"
                >
                  この条件で絞り込む
                </button>
                {onAskAgent != null && (
                  <button
                    type="button"
                    onClick={() => {
                      const filters = readDraft(draft)
                      const error = getScreenerRangeError(filters)
                      setValidationError(error)
                      if (error != null) return
                      setMobileFiltersOpen(false)
                      onAskAgent({ ...search, ...filters })
                    }}
                    className="flex w-full items-center justify-center gap-1 border border-primary bg-background px-3 py-3 font-semibold text-primary hover:bg-primary/10"
                  >
                    <span aria-hidden="true" className="font-bold">
                      &gt;_
                    </span>
                    この条件でエージェントに頼む
                  </button>
                )}
              </div>
            </form>
          </section>
        </div>
      )}
    </section>
  )
}
