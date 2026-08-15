import { cleanup, render, screen } from '@testing-library/react'
import { ReactFlowProvider } from '@xyflow/react'
import { afterEach, describe, expect, it } from 'vitest'

import { buildNodeProps } from '#components/graph/flow-node-props.test-helper'
import { GraphNodeView } from '#components/graph/graph-node'
import {
  GraphRenderContextProvider,
  type GraphRenderContextValue,
} from '#components/graph/graph-render-context'
import type { GraphNode, Layout } from '#components/graph/types'

afterEach(cleanup)

// GraphNodeView は内部で Handle (@xyflow/react) を使うため ReactFlowProvider が要る
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
  return render(
    <ReactFlowProvider>
      <GraphRenderContextProvider value={value}>
        <GraphNodeView {...buildNodeProps(data, 'graphNode')} />
      </GraphRenderContextProvider>
    </ReactFlowProvider>,
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
