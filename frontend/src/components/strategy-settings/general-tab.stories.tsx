import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { GeneralTab } from '#components/strategy-settings/general-tab'
import { fetchClient } from '#lib/api/client'
import { createStoryRouter } from '#storybook/story-router'

interface Strategy {
  id: string
  name: string
  description: string | null
}

// Storybook にはグローバルな QueryClientProvider が無いため、$api.useQuery を使う
// GeneralTab 用にこのファイルの story 全体でモックを用意する
function installMiddleware(strategy: Strategy, patchStatus: number) {
  const state = { ...strategy }
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url, method } = request
      if (/\/api\/strategies\/[^/]+$/.test(url)) {
        if (method === 'GET') {
          return new Response(JSON.stringify(state), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PATCH') {
          if (patchStatus !== 200) {
            return new Response(
              JSON.stringify({ error: '更新に失敗しました' }),
              {
                status: patchStatus,
                headers: { 'content-type': 'application/json' },
              },
            )
          }
          return request
            .clone()
            .json()
            .then((body: Partial<Strategy>) => {
              Object.assign(state, body)
              return new Response(JSON.stringify(state), {
                status: 200,
                headers: { 'content-type': 'application/json' },
              })
            })
        }
        if (method === 'DELETE') {
          return new Response(null, { status: 204 })
        }
      }
      if (method === 'GET' && /\/api\/strategies(\?|$)/.test(url)) {
        return new Response(JSON.stringify([state]), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      return new Response(`unmocked request: ${method} ${url}`, {
        status: 404,
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function createGeneralTabRouter(strategy: Strategy, patchStatus = 200) {
  return createStoryRouter(
    () => <QueryDecorator strategy={strategy} patchStatus={patchStatus} />,
    { paths: ['/strategies'] },
  )
}

function QueryDecorator({
  strategy,
  patchStatus,
}: {
  strategy: Strategy
  patchStatus: number
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware(strategy, patchStatus))
  useEffect(() => eject, [eject])
  return (
    <QueryClientProvider client={client}>
      <div className="max-w-lg p-6">
        <GeneralTab strategyId={strategy.id} />
      </div>
    </QueryClientProvider>
  )
}

const meta = {
  title: 'StrategySettings/GeneralTab',
} satisfies Meta

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <RouterProvider
      router={createGeneralTabRouter({
        id: 'strat-1',
        name: '半導体短期スイング',
        description: '半導体セクターの短期変動を狙う',
      })}
    />
  ),
}

export const NoDescription: Story = {
  render: () => (
    <RouterProvider
      router={createGeneralTabRouter({
        id: 'strat-1',
        name: '長期投資',
        description: null,
      })}
    />
  ),
}
