import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { GraphRenderer } from '#components/graph/graph-renderer'
import type { GraphDef } from '#components/graph/types'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

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
  it('帰属表記の背景を透明にする', () => {
    render(<GraphRenderer def={FLOW_DEF} />)

    expect(
      document
        .querySelector<HTMLElement>('.react-flow')
        ?.style.getPropertyValue('--xy-attribution-background-color'),
    ).toBe('transparent')
  })

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
      { id: 'group', label: 'Group' },
      { id: 'a', label: 'A', parent: 'group' },
      { id: 'b', label: 'B' },
      { id: 'c', label: 'C' },
    ],
    edges: [{ source: 'a', target: 'b' }],
  }

  function highlightClasses(node: Element | null) {
    return Array.from(node?.classList ?? []).filter((className) =>
      ['opacity-100', 'opacity-30', 'transition-opacity'].includes(className),
    )
  }

  function highlightState(...nodes: (Element | null)[]) {
    return nodes.map(highlightClasses)
  }

  function renderHoverNodes() {
    render(<GraphRenderer def={HOVER_DEF} />)
    return {
      nodeA: screen.getByText('A').closest('[data-slot="graph-node"]'),
      nodeB: screen.getByText('B').closest('[data-slot="graph-node"]'),
      nodeC: screen.getByText('C').closest('[data-slot="graph-node"]'),
      nodeGroup: document.querySelector('.react-flow__node-graphGroup > div'),
      nodeAWrapper: screen.getByTestId('rf__node-a'),
    }
  }

  it('hover した node の 1 hop 隣接ノードだけハイライトする', () => {
    const { nodeA, nodeB, nodeC, nodeGroup, nodeAWrapper } = renderHoverNodes()

    fireEvent.mouseEnter(nodeAWrapper)

    expect(highlightState(nodeA, nodeB, nodeC, nodeGroup)).toEqual([
      ['opacity-100'],
      ['opacity-100'],
      ['opacity-30', 'transition-opacity'],
      ['opacity-30', 'transition-opacity'],
    ])
  })

  it('hover した group node 自身をハイライトする', () => {
    const { nodeA, nodeB, nodeC, nodeGroup } = renderHoverNodes()

    fireEvent.mouseEnter(screen.getByTestId('rf__node-group'))

    expect(highlightState(nodeA, nodeB, nodeC, nodeGroup)).toEqual([
      ['opacity-30', 'transition-opacity'],
      ['opacity-30', 'transition-opacity'],
      ['opacity-30', 'transition-opacity'],
      ['opacity-100'],
    ])
  })

  it('mouseLeave で元のハイライトに戻す', () => {
    const { nodeA, nodeB, nodeC, nodeGroup, nodeAWrapper } = renderHoverNodes()

    fireEvent.mouseEnter(nodeAWrapper)
    fireEvent.mouseLeave(nodeAWrapper)

    expect(highlightState(nodeA, nodeB, nodeC, nodeGroup)).toEqual([
      ['opacity-100'],
      ['opacity-100'],
      ['opacity-100'],
      ['opacity-100'],
    ])
  })

  it('hover しても計測済みの node は visible のまま', () => {
    const width = 240
    const height = 80
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(width)
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(
      height,
    )
    // 初回だけ同期計測し、再登録時の通知前に hover 直後の状態を確認する
    const measuredNodes = new WeakSet<Element>()
    const contentRect: DOMRectReadOnly = {
      x: 0,
      y: 0,
      width,
      height,
      top: 0,
      right: width,
      bottom: height,
      left: 0,
      toJSON: () => ({}),
    }
    const size: ResizeObserverSize = { inlineSize: width, blockSize: height }

    class ImmediateResizeObserver implements ResizeObserver {
      constructor(private readonly callback: ResizeObserverCallback) {}

      observe(target: Element): void {
        if (!target.hasAttribute('data-id') || measuredNodes.has(target)) return
        measuredNodes.add(target)
        this.callback(
          [
            {
              target,
              contentRect,
              borderBoxSize: [size],
              contentBoxSize: [size],
              devicePixelContentBoxSize: [size],
            },
          ],
          this,
        )
      }

      unobserve() {}

      disconnect() {}
    }
    vi.stubGlobal('ResizeObserver', ImmediateResizeObserver)
    vi.stubGlobal(
      'DOMMatrixReadOnly',
      class IdentityDOMMatrixReadOnly {
        readonly m22 = 1
      },
    )

    render(<GraphRenderer def={HOVER_DEF} />)
    const nodeA = screen.getByTestId('rf__node-a')
    const visibilityStates = [nodeA.style.visibility]

    fireEvent.mouseEnter(nodeA)
    visibilityStates.push(nodeA.style.visibility)

    expect(visibilityStates).toEqual(['visible', 'visible'])
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
    expect(background).not.toBeNull()
    if (background == null) return
    const nodeA = screen.getByText('点A').closest('[data-slot="graph-node"]')
    const nodeB = screen.getByText('点B').closest('[data-slot="graph-node"]')

    // データノードが無い領域 (= 背景ノード) を hover しても、実データノードは
    // dim されない (hoveredId が背景ノードの id になってしまうバグの回帰確認)
    fireEvent.mouseEnter(background)
    expect(backgroundHoverState(background, nodeA, nodeB)).toEqual([
      true,
      ['opacity-100'],
      ['opacity-100'],
    ])
  })

  function backgroundHoverState(
    background: Element,
    nodeA: Element | null,
    nodeB: Element | null,
  ) {
    return [
      background.classList.contains('pointer-events-none'),
      ...highlightState(nodeA, nodeB),
    ]
  }
})
