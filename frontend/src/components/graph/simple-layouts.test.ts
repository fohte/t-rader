import { describe, expect, it } from 'vitest'

import {
  buildChainLayout,
  buildScatterLayout,
  SCATTER_BACKGROUND_ID,
} from '#components/graph/simple-layouts'
import type { GraphDef } from '#components/graph/types'

describe('buildChainLayout', () => {
  it('存在しない参照があれば err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'chain',
      nodes: [{ id: 'a', label: 'A' }],
      edges: [{ source: 'a', target: 'missing' }],
    }
    expect(buildChainLayout(def, new Map()).isErr()).toBe(true)
  })

  it('配列順のまま横一列に、value に応じた幅で x を積み上げて並べる', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'chain',
      nodes: [
        { id: 'a', label: 'A' },
        { id: 'b', label: 'B', value: 60 },
        { id: 'c', label: 'C' },
      ],
      edges: [{ source: 'a', target: 'b' }],
    }
    const result = buildChainLayout(def, new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return

    expect(result.value).toEqual({
      nodes: [
        {
          id: 'a',
          type: 'graphNode',
          position: { x: 0, y: 0 },
          data: { id: 'a', label: 'A' },
        },
        {
          id: 'b',
          type: 'graphNode',
          position: { x: 224, y: 0 },
          data: { id: 'b', label: 'B', value: 60 },
        },
        {
          id: 'c',
          type: 'graphNode',
          position: { x: 496, y: 0 },
          data: { id: 'c', label: 'C' },
        },
      ],
      edges: [
        {
          id: 'a-b-0',
          source: 'a',
          target: 'b',
          label: undefined,
          style: { strokeWidth: 1.5 },
          data: { source: 'a', target: 'b' },
        },
      ],
    })
  })
})

// scale() の除算・乗算の順序に起因する 2 進浮動小数点誤差 (例: 452.00000000000006) を
// 丸めて消す。挙動を変えないリファクタで下位桁がずれるだけの red を防ぐ
function roundPosition<T extends { position: { x: number; y: number } }>(
  node: T,
): T {
  return {
    ...node,
    position: {
      x: Math.round(node.position.x * 1e6) / 1e6,
      y: Math.round(node.position.y * 1e6) / 1e6,
    },
  }
}

describe('buildScatterLayout', () => {
  it('存在しない参照があれば err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'scatter',
      nodes: [{ id: 'a', label: 'A' }],
      edges: [{ source: 'a', target: 'missing' }],
    }
    expect(buildScatterLayout(def, new Map()).isErr()).toBe(true)
  })

  it('背景ノードを先頭に置き、x/y を軸スケールで px に変換する (y は反転)', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'scatter',
      nodes: [
        { id: 'a', label: 'A', x: 0, y: 0 },
        { id: 'b', label: 'B', x: 100, y: 100 },
      ],
      edges: [],
    }
    const result = buildScatterLayout(def, new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return

    // x=0,y=0 / x=100,y=100 → domain [-20,120] の 1/7 点。y は invert するため上下が反転する
    expect(result.value.nodes.map(roundPosition)).toEqual([
      {
        id: SCATTER_BACKGROUND_ID,
        type: 'graphScatterBackground',
        position: { x: 0, y: 0 },
        style: { width: 560, height: 560 },
        zIndex: -1,
        selectable: false,
        draggable: false,
        connectable: false,
        data: {},
      },
      {
        id: 'a',
        type: 'graphNode',
        position: { x: 0, y: 452 },
        data: { id: 'a', label: 'A', x: 0, y: 0 },
      },
      {
        id: 'b',
        type: 'graphNode',
        position: { x: 400, y: 52 },
        data: { id: 'b', label: 'B', x: 100, y: 100 },
      },
    ])
    expect(result.value.edges).toEqual([])
  })
})
