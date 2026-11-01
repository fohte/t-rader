import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type PendingNoteVersion = components['schemas']['PendingNoteVersion']

function strategyStub(id: string, name: string, sortOrder: number): Strategy {
  return {
    id,
    name,
    description: null,
    sort_order: sortOrder,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  }
}

const STRATEGIES: Strategy[] = [
  strategyStub('sample-alpha', '架空戦略 A', 0),
  strategyStub('sample-beta', '架空戦略 B', 1),
]

function unreadNoteVersion(index: number): PendingNoteVersion {
  const suffix = String(index).padStart(12, '0')
  return {
    body_md: '',
    created_at: '2026-01-01T00:00:00Z',
    created_by_kind: 'llm',
    frontmatter_json: {},
    graphs_json: [],
    id: `00000000-0000-4000-8000-${suffix}`,
    is_current: true,
    kind: null,
    resolved_price_references_json: {},
    warning_dispositions: [],
    status: 'unread',
    tags: [],
    title: `デモ戦略のノート ${String(index)}`,
    note_id: `10000000-0000-4000-8000-${suffix}`,
    version_no: 1,
  }
}

// Storybook にはグローバルな QueryClientProvider が無いため、レビュー件数をモックする
function installMiddleware(unreadCount: number, reviewQueueError: boolean) {
  const unreadVersions = Array.from({ length: unreadCount }, (_, index) =>
    unreadNoteVersion(index + 1),
  )
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      if (
        reviewQueueError &&
        /\/api\/(notes|note-versions\/pending|annotations)(\?|$)/.test(url)
      ) {
        return new Response(JSON.stringify({ message: 'fixture error' }), {
          status: 500,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/strategies(\?|$)/.test(url)) {
        return new Response(JSON.stringify(STRATEGIES), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/note-versions\/pending(\?|$)/.test(url)) {
        return new Response(JSON.stringify(unreadVersions), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/annotations(\?|$)/.test(url)) {
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

// StrategySwitcher と Header を描画する story に QueryClientProvider と上記モックを与える。
export function StrategySwitcherQueryDecorator({
  children,
  unreadCount = 2,
  reviewQueueError = false,
}: {
  children: React.ReactNode
  unreadCount?: number
  reviewQueueError?: boolean
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() =>
    installMiddleware(unreadCount, reviewQueueError),
  )
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}
