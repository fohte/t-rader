import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen } from '@testing-library/react'
import { ReactFlowProvider } from '@xyflow/react'
import type { Middleware } from 'openapi-fetch'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { buildNodeProps } from '#components/graph/flow-node-props.test-helper'
import { GraphNodeView } from '#components/graph/graph-node'
import {
  GraphRenderContextProvider,
  type GraphRenderContextValue,
} from '#components/graph/graph-render-context'
import type { GraphNode, Layout } from '#components/graph/types'
import { fetchClient } from '#lib/api/client'

afterEach(cleanup)

// RefChip が使う $api.useQuery('/api/refs/resolve') 用のモック。常に未解決を返す
const refResolveMiddleware: Middleware = {
  onRequest({ request }) {
    if (!/\/api\/refs\/resolve(\?|$)/.test(request.url)) return undefined
    return new Response('[]', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    })
  },
}
beforeEach(() => {
  fetchClient.use(refResolveMiddleware)
})
afterEach(() => {
  fetchClient.eject(refResolveMiddleware)
})

// GraphNodeView は内部で Handle (@xyflow/react) を使うため ReactFlowProvider が、
// RefChip が $api.useQuery を使うため QueryClientProvider が要る
function renderNode(
  data: GraphNode,
  context: Partial<GraphRenderContextValue> = {},
) {
  const value: GraphRenderContextValue = {
    layout: 'flow',
    maxNodeValue: 100,
    citeNumbers: new Map(),
    ...context,
  }
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>
      <ReactFlowProvider>
        <GraphRenderContextProvider value={value}>
          <GraphNodeView {...buildNodeProps(data, 'graphNode')} />
        </GraphRenderContextProvider>
      </ReactFlowProvider>
    </QueryClientProvider>,
  )
}

describe('GraphNodeView', () => {
  it('label を表示する', () => {
    renderNode({ id: 'a', label: 'ノードA' })
    expect(screen.getByText('ノードA')).toBeInTheDocument()
  })

  it('ref があれば RefChip (トークンが解決された表示) を出す', () => {
    renderNode({ id: 'a', label: 'A', ref: 'stock:ACME' })
    expect(screen.getByText('ACME')).toBeInTheDocument()
  })

  it('ref が無ければ RefChip を出さない', () => {
    const { container } = renderNode({ id: 'a', label: 'A' })
    expect(container.querySelector('[data-kind]')).toBeNull()
  })

  it('cite があり citeNumbers に対応する番号があればバッジを出す', () => {
    renderNode(
      { id: 'a', label: 'A', cite: '出典1' },
      { citeNumbers: new Map([['出典1', 3]]) },
    )
    expect(screen.getByText('3')).toBeInTheDocument()
  })

  it('cite があっても citeNumbers に対応する番号が無ければバッジを出さない', () => {
    const { container } = renderNode(
      { id: 'a', label: 'A', cite: '出典1' },
      { citeNumbers: new Map() },
    )
    expect(container.querySelector('button')).toBeNull()
  })

  it.each<[Layout, boolean]>([
    ['chain', true],
    ['flow', false],
    ['tree', false],
    ['scatter', false],
  ])(
    'layout=%s かつ value ありのとき棒グラフ要素が出るのは chain のみ (出る: %s)',
    (layout, expectBar) => {
      const { container } = renderNode(
        { id: 'a', label: 'A', value: 50 },
        { layout, maxNodeValue: 100 },
      )
      expect(container.querySelectorAll('.bg-primary').length).toBe(
        expectBar ? 1 : 0,
      )
    },
  )

  it('layout=chain でも value が無ければ棒グラフ要素は出ない', () => {
    const { container } = renderNode(
      { id: 'a', label: 'A' },
      { layout: 'chain', maxNodeValue: 100 },
    )
    expect(container.querySelectorAll('.bg-primary').length).toBe(0)
  })
})
