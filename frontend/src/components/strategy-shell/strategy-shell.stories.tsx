import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Outlet, RouterProvider } from '@tanstack/react-router'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { StrategyShell } from '#components/strategy-shell/strategy-shell'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type Strategy = components['schemas']['Strategy']

const STRATEGIES: Strategy[] = [
  {
    id: 'semi-swing',
    name: '半導体短期スイング',
    description: null,
    sort_order: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 'value-long',
    name: '高配当バリュー長期',
    description: null,
    sort_order: 1,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

// StrategyShell (Header 経由) にはグローバルな QueryClientProvider が無いため、
// StrategySwitcher が使う $api.useQuery 用にこのファイルの story 全体でモックを用意する
function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      if (/\/api\/strategies(\?|$)/.test(url)) {
        return new Response(JSON.stringify(STRATEGIES), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/notes(\?|$)/.test(url)) {
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      return new Response(`unmocked request: ${request.method} ${url}`, {
        status: 404,
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function QueryDecorator({ children }: { children: React.ReactNode }) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

const PLACEHOLDER_PATHS = [
  { path: '/strategies', label: '戦略一覧 placeholder' },
  { path: '/strategies/$id', label: '戦略ホーム placeholder' },
  { path: '/portfolio', label: 'ポートフォリオ placeholder' },
  { path: '/trades', label: '取引履歴 placeholder' },
].map(({ path, label }) => ({
  path,
  component: () => (
    <div className="font-mono text-sm text-[color:var(--color-text-secondary)]">
      {label}
    </div>
  ),
}))

function createStrategyShellRouter(initialPath: string) {
  return createStoryRouter(
    () => (
      <StrategyShell>
        <Outlet />
      </StrategyShell>
    ),
    { paths: PLACEHOLDER_PATHS, initialPath },
  )
}

const meta = {
  title: 'StrategyShell/StrategyShell',
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <QueryDecorator>
        <Story />
      </QueryDecorator>
    ),
  ],
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <RouterProvider router={createStrategyShellRouter('/strategies')} />
  ),
}

export const StrategyHome: Story = {
  render: () => (
    <RouterProvider
      router={createStrategyShellRouter('/strategies/semi-swing')}
    />
  ),
}
