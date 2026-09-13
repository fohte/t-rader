import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import { useEffect, useState } from 'react'

import { fetchClient } from '#lib/api/client'

// Storybook にはグローバルな QueryClientProvider が無いため、RefChip が使う
// $api.useQuery('/api/refs/resolve') 用にモックを用意する。常に未解決 (name:
// null 相当の空配列) を返し、RefChip 側のフォールバック表示に委ねる。
function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      if (!/\/api\/refs\/resolve(\?|$)/.test(request.url)) return undefined
      return new Response('[]', {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

// RefChip (直接 or markdown/graph 経由) を描画する story に QueryClientProvider と
// 上記モックをまとめて与える decorator。
export function RefResolveQueryDecorator({
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
