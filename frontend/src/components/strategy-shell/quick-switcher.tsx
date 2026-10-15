import type {
  CommandPaletteGroup,
  CommandPaletteItem,
} from '@fohte/ui/command-palette'
import { useRouter } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'

import {
  QuickSwitcherItem,
  QuickSwitcherView,
} from '#components/strategy-shell/quick-switcher-view'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']
type Indicator = components['schemas']['Indicator']
type AppRouter = ReturnType<typeof useRouter>

const SCREEN_ITEMS = [
  {
    id: 'dashboard',
    label: 'ダッシュボード',
    searchTerms: ['dashboard', 'home'],
    to: '/',
  },
  {
    id: 'portfolio',
    label: '資産 · 保有',
    searchTerms: ['資産', '保有', 'portfolio'],
    to: '/portfolio',
  },
  {
    id: 'trades',
    label: '資産 · 取引',
    searchTerms: ['資産', '取引', 'trades'],
    to: '/trades',
  },
  {
    id: 'review',
    label: 'レビュー',
    searchTerms: ['review'],
    to: '/review',
  },
  {
    id: 'notes',
    label: 'ノート',
    searchTerms: ['notes'],
    to: '/notes',
  },
  {
    id: 'annotations',
    label: 'アノテーション',
    searchTerms: ['annotation', 'annotations'],
    to: '/notes',
    search: { tab: 'annotations' },
  },
  {
    id: 'strategies',
    label: '戦略一覧',
    searchTerms: ['strategies'],
    to: '/strategies',
  },
  {
    id: 'strategy-runs',
    label: '実行履歴',
    searchTerms: ['戦略', '実行', '履歴', 'runs'],
    to: '/strategies',
    search: { strategyTab: 'runs' },
  },
  {
    id: 'paper-trading',
    label: 'ペーパートレード',
    searchTerms: ['paper', 'trading'],
    to: '/paper-trading',
  },
  {
    id: 'calendar',
    label: 'カレンダー',
    searchTerms: ['calendar'],
    to: '/calendar',
  },
  {
    id: 'settings',
    label: '全体の設定',
    searchTerms: ['設定', 'settings'],
    to: '/settings',
  },
  {
    id: 'agent-configs',
    label: 'Agent 設定',
    searchTerms: ['設定', 'agent', 'agents'],
    to: '/settings/agent-configs',
  },
  {
    id: 'rss-feeds',
    label: 'RSS フィード',
    searchTerms: ['設定', 'rss', 'feed'],
    to: '/settings/rss-feeds',
  },
  {
    id: 'ingest-status',
    label: '取り込み状況',
    searchTerms: ['設定', 'ingest', 'status'],
    to: '/settings/ingest-status',
  },
  {
    id: 'risk-policy',
    label: 'リスク上限',
    searchTerms: ['設定', 'risk', 'limit'],
    to: '/settings/risk-policy',
  },
  {
    id: 'custom-indicators',
    label: '自作指標',
    searchTerms: ['設定', '指標', 'indicator'],
    to: '/settings/indicators',
  },
] as const

type ScreenItem = (typeof SCREEN_ITEMS)[number]

function matchesQuery(query: string, values: string[]) {
  const normalizedQuery = query.toLocaleLowerCase('ja')
  return values.some((value) =>
    value.toLocaleLowerCase('ja').includes(normalizedQuery),
  )
}

function navigateToScreen(router: AppRouter, item: ScreenItem) {
  switch (item.id) {
    case 'annotations':
      void router.navigate({ to: item.to, search: item.search })
      return
    case 'strategy-runs':
      void router.navigate({ to: item.to, search: item.search })
      return
    default:
      void router.navigate({ to: item.to })
  }
}

function createItem(
  key: string,
  label: string,
  detail: string | undefined,
  onSelect: () => void,
): CommandPaletteItem {
  return {
    key,
    select: onSelect,
    render: ({ isSelected, onMouseMove }) => (
      <QuickSwitcherItem
        label={label}
        detail={detail}
        isSelected={isSelected}
        onMouseMove={onMouseMove}
        onSelect={onSelect}
      />
    ),
  }
}

function createGroup(
  id: string,
  title: string,
  items: CommandPaletteItem[],
): CommandPaletteGroup | undefined {
  return items.length > 0 ? { id, title, items } : undefined
}

function getShortcutPlatform() {
  if (typeof navigator === 'undefined') return 'other'
  return /Mac|iPhone|iPad|iPod/.test(
    `${navigator.platform} ${navigator.userAgent}`,
  )
    ? 'apple'
    : 'other'
}

export function QuickSwitcher() {
  const router = useRouter()
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState('')
  const shortcutPlatform = getShortcutPlatform()
  const shortcutLabel = shortcutPlatform === 'apple' ? '⌘K' : 'Ctrl+K'
  const normalizedQuery = value.trim()
  const hasQuery = normalizedQuery.length > 0
  const strategiesQuery = $api.useQuery('get', '/api/strategies', undefined, {
    enabled: open,
  })
  const stocksQuery = $api.useQuery(
    'get',
    '/api/refs/stocks',
    { params: { query: { q: normalizedQuery } } },
    { enabled: open && hasQuery },
  )
  const indicatorsQuery = $api.useQuery(
    'get',
    '/api/refs/indicators',
    { params: { query: { q: normalizedQuery } } },
    { enabled: open && hasQuery },
  )
  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen)
    if (!nextOpen) setValue('')
  }, [])

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.isComposing ||
        event.repeat ||
        event.key.toLocaleLowerCase() !== 'k'
      ) {
        return
      }
      const hasShortcutModifier =
        shortcutPlatform === 'apple' ? event.metaKey : event.ctrlKey
      if (
        !hasShortcutModifier ||
        event.altKey ||
        (shortcutPlatform === 'apple' ? event.ctrlKey : event.metaKey)
      ) {
        return
      }

      event.preventDefault()
      handleOpenChange(!open)
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [handleOpenChange, open, shortcutPlatform])

  const groups = useMemo(() => {
    const screenItems = SCREEN_ITEMS.filter(
      (item) =>
        !hasQuery ||
        matchesQuery(normalizedQuery, [item.label, ...item.searchTerms]),
    ).map((item) =>
      createItem(item.id, item.label, undefined, () => {
        handleOpenChange(false)
        navigateToScreen(router, item)
      }),
    )
    const strategies = (strategiesQuery.data ?? []).filter(
      (strategy: Strategy) =>
        !hasQuery ||
        matchesQuery(normalizedQuery, [strategy.name, strategy.id]),
    )
    const strategyItems = strategies.map((strategy: Strategy) =>
      createItem(`strategy:${strategy.id}`, strategy.name, strategy.id, () => {
        handleOpenChange(false)
        void router.navigate({
          to: '/strategies/$id/performance',
          params: { id: strategy.id },
        })
      }),
    )
    const stockItems = (stocksQuery.data ?? []).map((stock: Stock) =>
      createItem(`stock:${stock.id}`, stock.name, stock.id, () => {
        handleOpenChange(false)
        void router.navigate({
          to: '/stocks/$stockId',
          params: { stockId: stock.id },
        })
      }),
    )
    const indicatorItems = (indicatorsQuery.data ?? []).map(
      (indicator: Indicator) =>
        createItem(
          `indicator:${indicator.id}`,
          indicator.name,
          indicator.id,
          () => {
            handleOpenChange(false)
            void router.navigate({
              to: '/indicators/$indicatorId',
              params: { indicatorId: indicator.id },
            })
          },
        ),
    )

    return [
      createGroup('screens', '画面', screenItems),
      createGroup('strategies', '戦略', strategyItems),
      ...(hasQuery
        ? [
            createGroup('stocks', '銘柄', stockItems),
            createGroup('indicators', '指標', indicatorItems),
          ]
        : []),
    ].filter((group): group is CommandPaletteGroup => group != null)
  }, [
    handleOpenChange,
    hasQuery,
    indicatorsQuery.data,
    normalizedQuery,
    router,
    stocksQuery.data,
    strategiesQuery.data,
  ])

  const isLoading =
    open &&
    (strategiesQuery.isPending ||
      (hasQuery && (stocksQuery.isFetching || indicatorsQuery.isFetching)))
  const noResults = open && hasQuery && groups.length === 0 && !isLoading
  const searchFailed =
    open &&
    (strategiesQuery.isError ||
      (hasQuery && (stocksQuery.isError || indicatorsQuery.isError)))

  return (
    <QuickSwitcherView
      open={open}
      onOpenChange={handleOpenChange}
      shortcutLabel={shortcutLabel}
      value={value}
      onValueChange={setValue}
      groups={groups}
      loading={isLoading}
      emptyMessage={
        searchFailed
          ? groups.length > 0
            ? '一部の検索に失敗しました。'
            : '検索に失敗しました。'
          : noResults
            ? '該当する項目はありません。'
            : undefined
      }
    />
  )
}
