import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

import {
  installRefResolveMock,
  type RefResolveStub,
} from '#lib/refs.test-helper'

// 参照解決 API のモックと QueryClientProvider を提供する Storybook decorator。
// stubs で指定した token のみ解決済みとして返し、指定が無いものはフォールバック表示になる。
export function RefResolveQueryDecorator({
  stubs = [],
  children,
}: {
  stubs?: RefResolveStub[]
  children: React.ReactNode
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installRefResolveMock(stubs))
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}
