import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Outlet, RouterProvider } from '@tanstack/react-router'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { QuickSwitcher } from '#components/strategy-shell/quick-switcher'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Strategy = components['schemas']['Strategy']
type Stock = components['schemas']['Stock']
type Indicator = components['schemas']['Indicator']

const strategy: Strategy = {
  id: 'demo-strategy',
  name: 'デモ戦略',
  description: null,
  sort_order: 0,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const otherStrategy: Strategy = {
  id: 'demo-other-strategy',
  name: '別のデモ戦略',
  description: null,
  sort_order: 1,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const stock: Stock = {
  id: 'demo-code',
  name: 'デモ商事',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

const indicator: Indicator = {
  id: 'demo-indicator',
  kind: 'custom',
  name: 'デモ指標',
}

function installQuickSwitcherMiddleware(
  failingPath?: string,
  deferredQuery?: string,
) {
  const requests: { path: string; query: string | null }[] = []
  const deferredResponseResolvers: ((response: Response) => void)[] = []
  const middleware: Middleware = {
    onRequest({ request }) {
      const url = new URL(request.url)
      const path = url.pathname
      const query = url.searchParams.get('q')
      requests.push({ path, query })
      if (path === failingPath) {
        return new Response(JSON.stringify({ error: '検索に失敗しました。' }), {
          status: 500,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (query === deferredQuery) {
        return new Promise<Response>((resolve) => {
          deferredResponseResolvers.push(resolve)
        })
      }
      const body =
        path === '/api/strategies'
          ? [strategy, otherStrategy]
          : path === '/api/refs/stocks' && query === 'デモ'
            ? [stock]
            : path === '/api/refs/indicators' && query === 'デモ'
              ? [indicator]
              : []
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  return {
    requests,
    resolveDeferredQueries: () => {
      for (const resolve of deferredResponseResolvers.splice(0)) {
        resolve(
          new Response(JSON.stringify([]), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          }),
        )
      }
    },
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let activeMiddleware: ReturnType<typeof installQuickSwitcherMiddleware> | null =
  null

function setPlatform(platform: string) {
  Object.defineProperty(window.navigator, 'platform', {
    configurable: true,
    value: platform,
  })
}

function renderQuickSwitcher(
  initialPath = '/portfolio',
  platform = 'Linux',
  failingPath?: string,
  deferredQuery?: string,
) {
  activeMiddleware?.eject()
  activeMiddleware = installQuickSwitcherMiddleware(failingPath, deferredQuery)
  setPlatform(platform)

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createStoryRouter(
    () => (
      <>
        <QuickSwitcher />
        <Outlet />
      </>
    ),
    {
      paths: [
        { path: '/portfolio', component: () => <p>保有画面</p> },
        { path: '/notes', component: () => <p>ノート画面</p> },
        { path: '/strategies', component: () => <p>戦略一覧画面</p> },
        {
          path: '/strategies/$id/performance',
          component: () => <p>戦略画面</p>,
        },
        { path: '/stocks/$stockId', component: () => <p>銘柄画面</p> },
        {
          path: '/indicators/$indicatorId',
          component: () => <p>指標画面</p>,
        },
      ],
      initialPath,
    },
  )
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }
  render(<RouterProvider router={router} />, { wrapper: Wrapper })
  return router
}

function getRequestOutput() {
  return (activeMiddleware?.requests ?? [])
    .map(({ path, query }) => ({ path, query }))
    .sort((left, right) => left.path.localeCompare(right.path))
}

function getPaletteResultLabels() {
  return screen
    .queryAllByRole('option')
    .map((option) => option.querySelector('span')?.textContent ?? '')
}

function getDefaultPaletteOutput(dialog: HTMLElement) {
  return {
    groups: ['画面', '戦略'].filter((title) =>
      within(dialog).queryByText(title),
    ),
    resultCount: within(dialog).getAllByRole('option').length,
    shortcut: screen
      .getByRole('button', { name: '探す' })
      .getAttribute('title'),
  }
}

function getShortcutOutput(
  defaultOutput: ReturnType<typeof getDefaultPaletteOutput>,
) {
  return {
    defaultOutput,
    requestPaths: (activeMiddleware?.requests ?? []).map(({ path }) => path),
    closed: screen.queryByRole('dialog') == null,
  }
}

function getNavigationOutput(router: ReturnType<typeof createStoryRouter>) {
  return {
    location: {
      pathname: router.state.location.pathname,
      search: router.state.location.search,
    },
    closed: screen.queryByRole('dialog') == null,
  }
}

function getStrategyFilteringOutput(
  matchingResults: string[],
  router: ReturnType<typeof createStoryRouter>,
) {
  return {
    matchingResults,
    navigation: getNavigationOutput(router),
    strategyRequests: getRequestOutput().filter(
      ({ path }) => path === '/api/strategies',
    ),
  }
}

function getReferenceNavigationOutput(
  router: ReturnType<typeof createStoryRouter>,
) {
  return {
    pathname: router.state.location.pathname,
    referenceRequests: getRequestOutput().filter(({ path }) =>
      path.startsWith('/api/refs/'),
    ),
    closed: screen.queryByRole('dialog') == null,
  }
}

function getSearchFailureOutput(message: string) {
  return {
    message: screen.getByText(message).textContent,
    results: getPaletteResultLabels(),
    requests: getRequestOutput(),
  }
}

function getReferenceSearchOutput<T>(
  beforeDebounce: T,
  firstSearch: T,
  whileNextSearchIsPending: T,
) {
  return { beforeDebounce, firstSearch, whileNextSearchIsPending }
}

afterEach(() => {
  cleanup()
  activeMiddleware?.resolveDeferredQueries()
  activeMiddleware?.eject()
  activeMiddleware = null
})

describe('QuickSwitcher', () => {
  it.each([
    { platform: 'Linux x86_64', modifier: 'ctrlKey', label: 'Ctrl+K' },
    { platform: 'MacIntel', modifier: 'metaKey', label: '⌘K' },
  ] as const)(
    'opens and closes with the platform shortcut on $platform',
    async ({ platform, modifier, label }) => {
      const user = userEvent.setup()
      renderQuickSwitcher('/portfolio', platform)

      await screen.findByRole('button', { name: '探す' })
      document.dispatchEvent(
        new window.KeyboardEvent('keydown', {
          key: 'k',
          [modifier]: true,
        }),
      )
      const dialog = await screen.findByRole('dialog', { name: '探す' })
      await screen.findByText('デモ戦略')
      const defaultOutput = getDefaultPaletteOutput(dialog)
      await user.keyboard('{Escape}')

      expect(getShortcutOutput(defaultOutput)).toEqual({
        defaultOutput: {
          groups: ['画面', '戦略'],
          resultCount: 18,
          shortcut: `探す ${label}`,
        },
        requestPaths: ['/api/strategies'],
        closed: true,
      })
    },
  )

  it('filters strategies locally and navigates to the selected strategy', async () => {
    const user = userEvent.setup()
    const router = renderQuickSwitcher()

    await user.click(await screen.findByRole('button', { name: '探す' }))
    const input = await screen.findByRole('textbox', {
      name: '画面・戦略・銘柄・指標を検索',
    })
    await user.click(input)
    await user.paste('demo-strategy')
    await screen.findByText('デモ戦略')
    const matchingResults = getPaletteResultLabels()
    await user.click(screen.getByText('デモ戦略'))
    await screen.findByText('戦略画面')

    expect(getStrategyFilteringOutput(matchingResults, router)).toEqual({
      matchingResults: ['デモ戦略'],
      navigation: {
        location: {
          pathname: '/strategies/demo-strategy/performance',
          search: {},
        },
        closed: true,
      },
      strategyRequests: [{ path: '/api/strategies', query: null }],
    })
  })

  it('debounces reference searches and keeps prior results while the next search loads', async () => {
    const user = userEvent.setup()
    renderQuickSwitcher('/portfolio', 'Linux', undefined, 'デモX')

    await user.click(await screen.findByRole('button', { name: '探す' }))
    await screen.findByText('デモ戦略')
    const input = screen.getByRole('textbox', {
      name: '画面・戦略・銘柄・指標を検索',
    })

    fireEvent.change(input, { target: { value: 'デ' } })
    fireEvent.change(input, { target: { value: 'デモ' } })
    await new Promise((resolve) => window.setTimeout(resolve, 150))
    const beforeDebounce = {
      referenceRequests: getRequestOutput().filter(({ path }) =>
        path.startsWith('/api/refs/'),
      ),
      results: getPaletteResultLabels(),
    }

    await screen.findByText('デモ商事')
    const firstSearch = {
      referenceRequests: getRequestOutput().filter(({ path }) =>
        path.startsWith('/api/refs/'),
      ),
      results: getPaletteResultLabels(),
    }

    fireEvent.change(input, { target: { value: 'デモX' } })
    await new Promise((resolve) => window.setTimeout(resolve, 220))
    const whileNextSearchIsPending = {
      referenceRequests: getRequestOutput().filter(({ path }) =>
        path.startsWith('/api/refs/'),
      ),
      results: getPaletteResultLabels(),
    }

    const expected = {
      beforeDebounce: {
        referenceRequests: [],
        results: ['デモ戦略', '別のデモ戦略'],
      },
      firstSearch: {
        referenceRequests: [
          { path: '/api/refs/indicators', query: 'デモ' },
          { path: '/api/refs/stocks', query: 'デモ' },
        ],
        results: ['デモ戦略', '別のデモ戦略', 'デモ商事', 'デモ指標'],
      },
      whileNextSearchIsPending: {
        referenceRequests: [
          { path: '/api/refs/indicators', query: 'デモ' },
          { path: '/api/refs/indicators', query: 'デモX' },
          { path: '/api/refs/stocks', query: 'デモ' },
          { path: '/api/refs/stocks', query: 'デモX' },
        ],
        results: ['デモ商事', 'デモ指標'],
      },
    }

    expect(
      getReferenceSearchOutput(
        beforeDebounce,
        firstSearch,
        whileNextSearchIsPending,
      ),
    ).toEqual(expected)
  })

  it.each([
    {
      query: '設定',
      expectedMessage: '一部の検索に失敗しました。',
      expectedResults: [
        '全体の設定',
        'Agent 設定',
        'RSS フィード',
        '取り込み状況',
        'リスク上限',
        '自作指標',
      ],
    },
    {
      query: '架空検索語',
      expectedMessage: '検索に失敗しました。',
      expectedResults: [],
    },
  ])(
    'shows an error when reference search fails for $query',
    async ({ query, expectedMessage, expectedResults }) => {
      const user = userEvent.setup()
      renderQuickSwitcher('/portfolio', 'Linux', '/api/refs/stocks')

      await user.click(await screen.findByRole('button', { name: '探す' }))
      const input = await screen.findByRole('textbox', {
        name: '画面・戦略・銘柄・指標を検索',
      })
      await user.click(input)
      await user.paste(query)
      await screen.findByText(expectedMessage)

      expect(getSearchFailureOutput(expectedMessage)).toEqual({
        message: expectedMessage,
        results: expectedResults,
        requests: [
          { path: '/api/refs/indicators', query },
          { path: '/api/refs/stocks', query },
          { path: '/api/strategies', query: null },
        ],
      })
    },
  )

  it.each([
    {
      query: 'アノテーション',
      label: 'アノテーション',
      page: 'ノート画面',
      pathname: '/notes',
      search: { tab: 'annotations' },
    },
    {
      query: '実行履歴',
      label: '実行履歴',
      page: '戦略一覧画面',
      pathname: '/strategies',
      search: { strategyTab: 'runs' },
    },
  ])(
    'navigates to the $label screen with its search parameters',
    async ({ query, label, page, pathname, search }) => {
      const user = userEvent.setup()
      const router = renderQuickSwitcher()

      await user.click(await screen.findByRole('button', { name: '探す' }))
      const input = await screen.findByRole('textbox', {
        name: '画面・戦略・銘柄・指標を検索',
      })
      await user.click(input)
      await user.paste(query)
      await user.click(await screen.findByText(label))
      await screen.findByText(page)

      expect(getNavigationOutput(router)).toEqual({
        location: { pathname, search },
        closed: true,
      })
    },
  )

  it.each([
    {
      group: '銘柄',
      resultName: 'デモ商事',
      page: '銘柄画面',
      pathname: '/stocks/demo-code',
    },
    {
      group: '指標',
      resultName: 'デモ指標',
      page: '指標画面',
      pathname: '/indicators/demo-indicator',
    },
  ])(
    'searches the server for $group and navigates to the result',
    async ({ resultName, page, pathname }) => {
      const user = userEvent.setup()
      const router = renderQuickSwitcher()

      await user.click(await screen.findByRole('button', { name: '探す' }))
      const input = await screen.findByRole('textbox', {
        name: '画面・戦略・銘柄・指標を検索',
      })
      await user.click(input)
      await user.paste('デモ')
      await user.click(await screen.findByText(resultName))
      await screen.findByText(page)

      expect(getReferenceNavigationOutput(router)).toEqual({
        pathname,
        referenceRequests: [
          { path: '/api/refs/indicators', query: 'デモ' },
          { path: '/api/refs/stocks', query: 'デモ' },
        ],
        closed: true,
      })
    },
  )
})
