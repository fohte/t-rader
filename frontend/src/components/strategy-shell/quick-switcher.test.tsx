import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Outlet, RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, within } from '@testing-library/react'
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

function installQuickSwitcherMiddleware() {
  const requests: { path: string; query: string | null }[] = []
  const middleware: Middleware = {
    onRequest({ request }) {
      const url = new URL(request.url)
      const path = url.pathname
      const query = url.searchParams.get('q')
      requests.push({ path, query })
      const body =
        path === '/api/strategies'
          ? [strategy]
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

function renderQuickSwitcher(initialPath = '/portfolio', platform = 'Linux') {
  activeMiddleware?.eject()
  activeMiddleware = installQuickSwitcherMiddleware()
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

function getStrategyNavigationOutput(
  router: ReturnType<typeof createStoryRouter>,
) {
  return {
    ...getNavigationOutput(router),
    requests: getRequestOutput(),
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

afterEach(() => {
  cleanup()
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
          resultCount: 17,
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
    await user.click(await screen.findByText('デモ戦略'))
    await screen.findByText('戦略画面')

    expect(getStrategyNavigationOutput(router)).toEqual({
      location: {
        pathname: '/strategies/demo-strategy/performance',
        search: {},
      },
      requests: [
        { path: '/api/refs/indicators', query: 'demo-strategy' },
        { path: '/api/refs/stocks', query: 'demo-strategy' },
        { path: '/api/strategies', query: null },
      ],
      closed: true,
    })
  })

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
