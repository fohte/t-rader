import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

const STRATEGIES: Strategy[] = [
  {
    id: 'semi-swing',
    name: '半導体短期スイング',
    description: null,
    sort_order: 0,
    agents_md: '',
    skills: {},
    agent_graph: '',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
  {
    id: 'value-long',
    name: '高配当バリュー長期',
    description: null,
    sort_order: 1,
    agents_md: '',
    skills: {},
    agent_graph: '',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  },
]

// Storybook にはグローバルな QueryClientProvider が無いため、StrategySwitcher が
// 使う $api.useQuery('/api/strategies', '/api/notes') 用にモックを用意する
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

// StrategySwitcher (Header 経由) を描画する story に QueryClientProvider と
// 上記モックをまとめて与える decorator。
export function StrategySwitcherQueryDecorator({
  children,
}: {
  children: React.ReactNode
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware())
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}
