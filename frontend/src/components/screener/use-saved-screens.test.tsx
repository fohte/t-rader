import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { useSavedScreens } from '#components/screener/use-saved-screens'
import { fetchClient } from '#lib/api/client'
import {
  isScreenerSortBy,
  type SavedScreen,
  type SaveScreenRequest,
} from '#lib/screener-search'

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value)
}

function isRangeDto(
  value: unknown,
): value is { min?: number | null; max?: number | null } {
  return (
    isRecord(value) &&
    (!('min' in value) || value.min == null || typeof value.min === 'number') &&
    (!('max' in value) || value.max == null || typeof value.max === 'number')
  )
}

function isSaveScreenRequest(value: unknown): value is SaveScreenRequest {
  if (
    !isRecord(value) ||
    typeof value.name !== 'string' ||
    !isRecord(value.filters) ||
    !isRecord(value.sort)
  ) {
    return false
  }

  const { filters, sort } = value
  const rangeKeys = [
    'per',
    'pbr',
    'roe',
    'dividend_yield',
    'market_cap',
  ] as const

  return (
    Array.isArray(filters.markets) &&
    filters.markets.every((market) => typeof market === 'string') &&
    Array.isArray(filters.sectors) &&
    filters.sectors.every((sector) => typeof sector === 'string') &&
    rangeKeys.every(
      (key) => filters[key] === null || isRangeDto(filters[key]),
    ) &&
    typeof sort.field === 'string' &&
    isScreenerSortBy(sort.field) &&
    (sort.direction === 'asc' || sort.direction === 'desc')
  )
}

function makeSavedScreen(overrides: Partial<SavedScreen> = {}): SavedScreen {
  return {
    id: overrides.id ?? 'saved-screen-a',
    name: overrides.name ?? '試作条件',
    created_at: overrides.created_at ?? '2099-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2099-01-02T00:00:00Z',
    filters: overrides.filters ?? {
      markets: ['プライム'],
      sectors: ['sector-sample'],
      per: { min: null, max: 12 },
      pbr: null,
      roe: null,
      dividend_yield: null,
      market_cap: null,
    },
    sort: overrides.sort ?? { field: 'per', direction: 'asc' },
  }
}

type ApiFailure = {
  status: number
  message: string
}

type ApiFailures = Partial<
  Record<'get' | 'create' | 'update' | 'delete', ApiFailure>
>

function errorResponse(failure: ApiFailure) {
  return new Response(JSON.stringify({ error: failure.message }), {
    status: failure.status,
    headers: { 'content-type': 'application/json' },
  })
}

function installMiddleware(
  initial: SavedScreen[] = [],
  failures: ApiFailures = {},
) {
  const store = {
    screens: [...initial],
    createCalls: [] as SaveScreenRequest[],
    updateCalls: [] as Array<{ id: string; body: SaveScreenRequest }>,
    deleteCalls: [] as string[],
  }
  const middleware: Middleware = {
    async onRequest({ request }) {
      const url = new URL(request.url)
      const method = request.method.toUpperCase()
      if (url.pathname === '/api/saved-screens' && method === 'GET') {
        if (failures.get != null) return errorResponse(failures.get)
        return new Response(JSON.stringify(store.screens), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (url.pathname === '/api/saved-screens' && method === 'POST') {
        const body: unknown = await request.clone().json()
        if (!isSaveScreenRequest(body)) {
          return new Response(JSON.stringify({ error: 'invalid request' }), {
            status: 400,
            headers: { 'content-type': 'application/json' },
          })
        }
        store.createCalls.push(body)
        if (failures.create != null) return errorResponse(failures.create)
        const created = makeSavedScreen({
          id: 'saved-screen-b',
          name: body.name,
          filters: body.filters,
          sort: body.sort,
        })
        store.screens.push(created)
        return new Response(JSON.stringify(created), {
          status: 201,
          headers: { 'content-type': 'application/json' },
        })
      }

      const match = /^\/api\/saved-screens\/([^/]+)$/.exec(url.pathname)
      const id = match?.[1]
      if (id != null && method === 'PUT') {
        const body: unknown = await request.clone().json()
        if (!isSaveScreenRequest(body)) {
          return new Response(JSON.stringify({ error: 'invalid request' }), {
            status: 400,
            headers: { 'content-type': 'application/json' },
          })
        }
        store.updateCalls.push({ id, body })
        if (failures.update != null) return errorResponse(failures.update)
        const currentIndex = store.screens.findIndex(
          (screen) => screen.id === id,
        )
        const current = store.screens[currentIndex]
        if (current == null) {
          return new Response(JSON.stringify({ error: 'not found' }), {
            status: 404,
            headers: { 'content-type': 'application/json' },
          })
        }
        const updated = makeSavedScreen({ ...current, ...body })
        store.screens[currentIndex] = updated
        return new Response(JSON.stringify(updated), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (id != null && method === 'DELETE') {
        store.deleteCalls.push(id)
        if (failures.delete != null) return errorResponse(failures.delete)
        store.screens = store.screens.filter((screen) => screen.id !== id)
        return new Response(null, { status: 204 })
      }
      return new Response(JSON.stringify({ error: 'unmocked request' }), {
        status: 500,
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  return {
    store,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let activeMiddleware: ReturnType<typeof installMiddleware> | null = null

function setup(
  search: Parameters<typeof useSavedScreens>[0]['search'] = {},
  initial: SavedScreen[] = [],
  failures: ApiFailures = {},
) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial, failures)
  const onSelect = vi.fn()
  const onClearSelection = vi.fn()
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
  const hook = renderHook(
    () => useSavedScreens({ search, onSelect, onClearSelection }),
    { wrapper: Wrapper },
  )
  return { ...hook, onSelect, onClearSelection, queryClient }
}

function readListState(
  state: Pick<
    ReturnType<typeof useSavedScreens>,
    'listError' | 'listPending' | 'savedScreens'
  >,
) {
  return {
    listError: state.listError,
    listPending: state.listPending,
    savedScreens: state.savedScreens,
  }
}

function readMutationState(
  state: Pick<
    ReturnType<typeof useSavedScreens>,
    'error' | 'isPending' | 'savedScreens'
  >,
  onSelectCalls: unknown[][],
  onClearSelectionCalls: unknown[][],
) {
  return {
    error: state.error,
    isPending: state.isPending,
    savedScreens: state.savedScreens,
    onSelectCalls,
    onClearSelectionCalls,
  }
}

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
})

describe('useSavedScreens', () => {
  it('exposes a saved-screen list loading error', async () => {
    const { result } = setup({}, [], {
      get: { status: 500, message: '保存条件一覧の取得に失敗しました。' },
    })

    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: true,
        listPending: false,
        savedScreens: [],
      })
    })
  })

  it('clears a selected screen after the loaded list does not contain it', async () => {
    const { result, onClearSelection } = setup({
      screen: 'missing-screen',
      markets: ['プライム'],
      max_per: 12,
    })
    const initialListPending = result.current.listPending

    await waitFor(() => {
      const actual = JSON.stringify({
        clearSelectionCalls: onClearSelection.mock.calls,
        initialListPending,
        listPending: result.current.listPending,
        savedScreens: result.current.savedScreens,
      })
      const expected = JSON.stringify({
        clearSelectionCalls: [[]],
        initialListPending: true,
        listPending: true,
        savedScreens: [],
      })
      expect(actual).toBe(expected)
    })
  })

  it('keeps a selected screen when the list cannot be loaded', async () => {
    const { result, onClearSelection } = setup(
      { screen: 'missing-screen' },
      [],
      { get: { status: 500, message: '一覧を取得できません。' } },
    )

    await waitFor(() => {
      const actual = JSON.stringify({
        clearSelectionCalls: onClearSelection.mock.calls,
        listError: result.current.listError,
        listPending: result.current.listPending,
      })
      const expected = JSON.stringify({
        clearSelectionCalls: [],
        listError: true,
        listPending: false,
      })
      expect(actual).toBe(expected)
    })
  })

  it('creates a saved screen from the current filters and selects the response', async () => {
    const { result, onSelect } = setup()
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [],
      })
    })

    act(() => {
      result.current.onSave(
        '試作条件',
        { type: 'save' },
        { markets: ['プライム'], max_per: 12 },
      )
    })

    await waitFor(() => {
      expect(activeMiddleware?.store.createCalls).toEqual([
        {
          name: '試作条件',
          filters: {
            markets: ['プライム'],
            sectors: [],
            per: { max: 12 },
            pbr: null,
            roe: null,
            dividend_yield: null,
            market_cap: null,
          },
          sort: { field: 'market_cap', direction: 'desc' },
        },
      ])
    })
    await waitFor(() => {
      expect(onSelect.mock.calls).toEqual([
        [
          makeSavedScreen({
            id: 'saved-screen-b',
            name: '試作条件',
            filters: {
              markets: ['プライム'],
              sectors: [],
              per: { max: 12 },
              pbr: null,
              roe: null,
              dividend_yield: null,
              market_cap: null,
            },
            sort: { field: 'market_cap', direction: 'desc' },
          }),
        ],
      ])
    })
  })

  it('overwrites the selected screen with the current filter draft', async () => {
    const savedScreen = makeSavedScreen()
    const { result, onSelect } = setup(
      { screen: savedScreen.id, sort_by: 'per', sort_order: 'asc' },
      [savedScreen],
    )
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [savedScreen],
      })
    })

    act(() => {
      result.current.onOverwrite({ min_per: 5, max_per: 10 })
    })

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        {
          id: savedScreen.id,
          body: {
            name: savedScreen.name,
            filters: {
              markets: [],
              sectors: [],
              per: { min: 5, max: 10 },
              pbr: null,
              roe: null,
              dividend_yield: null,
              market_cap: null,
            },
            sort: { field: 'per', direction: 'asc' },
          },
        },
      ])
    })
    await waitFor(() => {
      expect(onSelect.mock.calls).toEqual([
        [
          makeSavedScreen({
            ...savedScreen,
            filters: {
              markets: [],
              sectors: [],
              per: { min: 5, max: 10 },
              pbr: null,
              roe: null,
              dividend_yield: null,
              market_cap: null,
            },
          }),
        ],
      ])
    })
  })

  it('renames a saved screen without replacing its conditions', async () => {
    const savedScreen = makeSavedScreen()
    const { result } = setup({}, [savedScreen])
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [savedScreen],
      })
    })

    act(() => {
      result.current.onSave('変更後の名前', { type: 'rename', savedScreen }, {})
    })

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        {
          id: savedScreen.id,
          body: {
            name: '変更後の名前',
            filters: savedScreen.filters,
            sort: savedScreen.sort,
          },
        },
      ])
    })
    await waitFor(() => {
      expect(result.current.savedScreens).toEqual([
        makeSavedScreen({ ...savedScreen, name: '変更後の名前' }),
      ])
    })
  })

  it('shows an update error without selecting the saved screen', async () => {
    const savedScreen = makeSavedScreen()
    const { result, onSelect, onClearSelection } = setup(
      { screen: savedScreen.id },
      [savedScreen],
      {
        update: {
          status: 409,
          message: '保存した条件を上書きできませんでした。',
        },
      },
    )
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [savedScreen],
      })
    })

    act(() => {
      result.current.onOverwrite({ min_per: 5, max_per: 10 })
    })

    await waitFor(() => {
      expect(
        readMutationState(
          result.current,
          onSelect.mock.calls,
          onClearSelection.mock.calls,
        ),
      ).toEqual({
        error: '保存した条件を上書きできませんでした。',
        isPending: false,
        savedScreens: [savedScreen],
        onSelectCalls: [],
        onClearSelectionCalls: [],
      })
    })
  })

  it('deletes a selected screen and clears its URL selection', async () => {
    const savedScreen = makeSavedScreen()
    const { result, onClearSelection } = setup({ screen: savedScreen.id }, [
      savedScreen,
    ])
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [savedScreen],
      })
    })

    act(() => {
      result.current.onDelete(savedScreen)
    })

    await waitFor(() => {
      expect(activeMiddleware?.store.deleteCalls).toEqual([savedScreen.id])
    })
    await waitFor(() => {
      expect(onClearSelection.mock.calls).toEqual([[]])
    })
  })

  it('keeps the selection when delete fails', async () => {
    const savedScreen = makeSavedScreen()
    const { result, onSelect, onClearSelection } = setup(
      { screen: savedScreen.id },
      [savedScreen],
      {
        delete: {
          status: 500,
          message: '保存した条件を削除できませんでした。',
        },
      },
    )
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [savedScreen],
      })
    })

    act(() => {
      result.current.onDelete(savedScreen)
    })

    await waitFor(() => {
      expect(
        readMutationState(
          result.current,
          onSelect.mock.calls,
          onClearSelection.mock.calls,
        ),
      ).toEqual({
        error: '保存した条件を削除できませんでした。',
        isPending: false,
        savedScreens: [savedScreen],
        onSelectCalls: [],
        onClearSelectionCalls: [],
      })
    })
  })

  it('shows the duplicate-name error returned by the API', async () => {
    const { result, onSelect } = setup({}, [], {
      create: {
        status: 409,
        message: '同じ名前の保存条件がすでにあります。',
      },
    })
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [],
      })
    })

    act(() => {
      result.current.onSave('試作条件', { type: 'save' }, {})
    })

    await waitFor(() => {
      expect(result.current.error).toBe('同じ名前の保存条件がすでにあります。')
    })
    expect(onSelect.mock.calls).toEqual([])
  })

  it('shows invalid ranges without sending a save request', async () => {
    const { result } = setup()
    await waitFor(() => {
      expect(readListState(result.current)).toEqual({
        listError: false,
        listPending: false,
        savedScreens: [],
      })
    })

    act(() => {
      result.current.onSave(
        '試作条件',
        { type: 'save' },
        {
          min_per: 20,
          max_per: 8,
        },
      )
    })

    await waitFor(() => {
      expect(result.current.error).toBe('下限は上限以下にしてください。')
    })
    expect(activeMiddleware?.store.createCalls).toEqual([])
  })
})
