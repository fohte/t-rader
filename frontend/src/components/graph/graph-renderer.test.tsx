import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import {
  afterAll,
  afterEach,
  beforeAll,
  describe,
  expect,
  it,
  vi,
} from 'vitest'

import { GraphRenderer } from '#components/graph/graph-renderer'
import type { GraphDef } from '#components/graph/types'

// jsdom には ResizeObserver が無いが React Flow がコンテナ計測に使う
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeAll(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})
afterAll(() => {
  vi.unstubAllGlobals()
})
afterEach(cleanup)

const FLOW_DEF: GraphDef = {
  id: 'g-flow',
  layout: 'flow',
  nodes: [
    { id: 'a', label: 'ノードA' },
    { id: 'b', label: 'ノードB' },
  ],
  edges: [{ source: 'a', target: 'b' }],
}

const TREE_DEF: GraphDef = {
  id: 'g-tree',
  layout: 'tree',
  nodes: [
    { id: 'root', label: 'ルート' },
    { id: 'child1', label: '子1' },
    { id: 'child2', label: '子2' },
  ],
  edges: [
    { source: 'root', target: 'child1' },
    { source: 'root', target: 'child2' },
  ],
}

const CHAIN_DEF: GraphDef = {
  id: 'g-chain',
  layout: 'chain',
  nodes: [
    { id: 'a', label: '工程A', value: 20 },
    { id: 'b', label: '工程B', value: 40 },
  ],
  edges: [{ source: 'a', target: 'b' }],
}

const SCATTER_DEF: GraphDef = {
  id: 'g-scatter',
  layout: 'scatter',
  nodes: [
    { id: 'a', label: '点A', x: 10, y: 20 },
    { id: 'b', label: '点B', x: 80, y: 90 },
  ],
  edges: [],
}

describe('GraphRenderer', () => {
  it.each<[string, GraphDef, string[]]>([
    ['flow', FLOW_DEF, ['ノードA', 'ノードB']],
    ['tree', TREE_DEF, ['ルート', '子1', '子2']],
    ['chain', CHAIN_DEF, ['工程A', '工程B']],
    ['scatter', SCATTER_DEF, ['点A', '点B']],
  ])('layout=%s: 全ノードの label が描画される', (_layout, def, labels) => {
    render(<GraphRenderer def={def} />)
    for (const label of labels) {
      expect(screen.getByText(label)).toBeInTheDocument()
    }
  })

  it.each<[string, GraphDef, string]>([
    [
      'edge が存在しない node を参照',
      {
        id: 'g-err-edge',
        layout: 'flow',
        nodes: [{ id: 'a', label: 'A' }],
        edges: [{ source: 'a', target: 'missing' }],
      },
      'graph "g-err-edge": edge.target = "missing" is not a known node id',
    ],
    [
      'parent が存在しない node を参照',
      {
        id: 'g-err-parent',
        layout: 'flow',
        nodes: [{ id: 'a', label: 'A', parent: 'missing' }],
        edges: [],
      },
      'graph "g-err-parent": node "a".parent = "missing" is not a known node id',
    ],
  ])(
    '%s のとき role=alert でエラーメッセージを表示する',
    (_desc, def, message) => {
      render(<GraphRenderer def={def} />)
      expect(screen.getByRole('alert').textContent).toBe(message)
    },
  )

  const HOVER_DEF: GraphDef = {
    id: 'g-hover',
    layout: 'flow',
    nodes: [
      { id: 'a', label: 'A' },
      { id: 'b', label: 'B' },
      { id: 'c', label: 'C' },
    ],
    edges: [{ source: 'a', target: 'b' }],
  }

  function renderHoverNodes() {
    render(<GraphRenderer def={HOVER_DEF} />)
    return {
      nodeA: screen.getByText('A').closest('.react-flow__node'),
      nodeB: screen.getByText('B').closest('.react-flow__node'),
      nodeC: screen.getByText('C').closest('.react-flow__node'),
    }
  }

  it('hover した node の 1 hop 隣接ノードだけハイライトする', () => {
    const { nodeA, nodeB, nodeC } = renderHoverNodes()
    expect(nodeA).not.toBeNull()
    expect(nodeB).not.toBeNull()
    expect(nodeC).not.toBeNull()
    if (nodeA == null || nodeB == null || nodeC == null) return

    fireEvent.mouseEnter(nodeA)
    // b は a と edge で繋がる 1 hop 隣接、c は非隣接
    expect(nodeA).toHaveClass('opacity-100')
    expect(nodeA).not.toHaveClass('opacity-30')
    expect(nodeB).toHaveClass('opacity-100')
    expect(nodeB).not.toHaveClass('opacity-30')
    expect(nodeC).toHaveClass('opacity-30', 'transition-opacity')
    expect(nodeC).not.toHaveClass('opacity-100')
  })

  it('mouseLeave で元のハイライトに戻す', () => {
    const { nodeA, nodeB, nodeC } = renderHoverNodes()
    expect(nodeA).not.toBeNull()
    expect(nodeB).not.toBeNull()
    expect(nodeC).not.toBeNull()
    if (nodeA == null || nodeB == null || nodeC == null) return

    fireEvent.mouseEnter(nodeA)
    fireEvent.mouseLeave(nodeA)
    expect(nodeA).toHaveClass('opacity-100')
    expect(nodeB).toHaveClass('opacity-100')
    expect(nodeC).toHaveClass('opacity-100')
    expect(nodeC).not.toHaveClass('opacity-30')
  })

  it('scatter layout の背景ノードは pointer-events-none で hover 判定から除外される', () => {
    const def: GraphDef = {
      id: 'g-scatter-hover',
      layout: 'scatter',
      nodes: [
        { id: 'a', label: '点A', x: 10, y: 20 },
        { id: 'b', label: '点B', x: 80, y: 90 },
      ],
      edges: [],
    }
    render(<GraphRenderer def={def} />)

    const background = document.querySelector(
      '.react-flow__node-graphScatterBackground',
    )
    const nodeA = screen.getByText('点A').closest('.react-flow__node')
    const nodeB = screen.getByText('点B').closest('.react-flow__node')
    expect(background).not.toBeNull()
    expect(nodeA).not.toBeNull()
    expect(nodeB).not.toBeNull()
    if (background == null || nodeA == null || nodeB == null) return

    expect(background).toHaveClass('pointer-events-none')

    // データノードが無い領域 (= 背景ノード) を hover しても、実データノードは
    // dim されない (hoveredId が背景ノードの id になってしまうバグの回帰確認)
    fireEvent.mouseEnter(background)
    expect(nodeA).toHaveClass('opacity-100')
    expect(nodeA).not.toHaveClass('opacity-30')
    expect(nodeB).toHaveClass('opacity-100')
    expect(nodeB).not.toHaveClass('opacity-30')
  })
})
