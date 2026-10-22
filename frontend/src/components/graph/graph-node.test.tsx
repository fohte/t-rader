import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ReactFlowProvider } from '@xyflow/react'
import type { Middleware } from 'openapi-fetch'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { buildNodeProps } from '#components/graph/flow-node-props.test-helper'
import { GraphNodeView } from '#components/graph/graph-node'
import {
  GraphRenderContextProvider,
  type GraphRenderContextValue,
} from '#components/graph/graph-render-context'
import type { GraphNode, Layout } from '#components/graph/types'
import { fetchClient } from '#lib/api/client'

afterEach(cleanup)

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
})

// GraphNodeView は内部で Handle (@xyflow/react) を使うため ReactFlowProvider が要る
function renderNode(
  data: GraphNode,
  context: Partial<GraphRenderContextValue> = {},
) {
  const value: GraphRenderContextValue = {
    layout: 'flow',
    maxNodeValue: 100,
    citeNumbers: new Map(),
    highlightedNodeIds: null,
    ...context,
  }
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

  it('ref があれば解決済みの名前を表示する', async () => {
    const middleware: Middleware = {
      onRequest() {
        return new Response(
          JSON.stringify([{ kind: 'stock', id: 'ACME', name: 'ACME Corp' }]),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      },
    }
    fetchClient.use(middleware)
    try {
      renderNode({ id: 'a', label: 'A', ref: 'stock:ACME' })
      expect(await screen.findByText('ACME Corp')).toBeInTheDocument()
    } finally {
      fetchClient.eject(middleware)
    }
  })

  it('id が空の stock ref でも onOpenRef があれば button にする', () => {
    const onOpenRef = vi.fn()
    renderNode({ id: 'a', label: 'A', ref: 'stock:' }, { onOpenRef })

    fireEvent.click(screen.getByRole('button'))

    expect(onOpenRef.mock.calls).toEqual([
      ['stock:', { kind: 'stock', id: '', name: null }],
    ])
  })

  it('ref の kind が不明なら /api/refs/resolve を呼ばずに id をそのまま表示する', () => {
    const middleware: Middleware = {
      onRequest() {
        throw new Error('/api/refs/resolve を呼んではいけない')
      },
    }
    fetchClient.use(middleware)
    try {
      renderNode({ id: 'a', label: 'A', ref: 'unknownkind:x' })
      expect(screen.getByText('x')).toBeInTheDocument()
    } finally {
      fetchClient.eject(middleware)
    }
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

  it('dagre のノード幅と高さを CSS 変数に渡す', () => {
    const { container } = renderNode({ id: 'a', label: 'A' })
    const node = container.querySelector<HTMLElement>(
      '[data-slot="graph-node"]',
    )

    expect(node?.style.cssText).toEqual(
      '--graph-node-width: 160px; --graph-node-min-height: 38px;',
    )
  })

  it('chain の値バーは高さを CSS 変数、幅を固定 class で表す', () => {
    const { container } = renderNode(
      { id: 'a', label: 'A', value: 50 },
      { layout: 'chain', maxNodeValue: 100 },
    )
    const bar = container.querySelector<HTMLElement>(
      '[data-slot="graph-node-value-bar"]',
    )

    expect(bar?.outerHTML).toEqual(
      '<div data-slot="graph-node-value-bar" class="bg-muted-foreground h-(--graph-node-bar-height) w-2" style="--graph-node-bar-height: 32px;"></div>',
    )
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
      expect(
        container.querySelectorAll('[data-slot="graph-node-value-bar"]').length,
      ).toBe(expectBar ? 1 : 0)
    },
  )

  it('layout=chain でも value が無ければ棒グラフ要素は出ない', () => {
    const { container } = renderNode(
      { id: 'a', label: 'A' },
      { layout: 'chain', maxNodeValue: 100 },
    )
    expect(
      container.querySelectorAll('[data-slot="graph-node-value-bar"]').length,
    ).toBe(0)
  })
})
