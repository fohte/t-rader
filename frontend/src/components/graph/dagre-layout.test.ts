import { describe, expect, it } from 'vitest'

import { buildDagreLayout } from '#components/graph/dagre-layout'
import type { GraphDef } from '#components/graph/types'

describe('buildDagreLayout', () => {
  it('存在しない参照があれば err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [{ id: 'a', label: 'A' }],
      edges: [{ source: 'a', target: 'missing' }],
    }
    const result = buildDagreLayout(def, 'LR', new Map())
    expect(result.isErr()).toBe(true)
  })

  it('rankdir=LR で group なしの 2 ノードを配置する', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'a', label: 'A' },
        { id: 'b', label: 'B' },
      ],
      edges: [{ source: 'a', target: 'b', label: 'つながり' }],
    }
    const result = buildDagreLayout(def, 'LR', new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return
    expect(result.value).toEqual({
      nodes: [
        {
          id: 'a',
          type: 'graphNode',
          position: { x: 0, y: 0 },
          parentId: undefined,
          extent: undefined,
          data: { id: 'a', label: 'A' },
        },
        {
          id: 'b',
          type: 'graphNode',
          position: { x: 310, y: 0 },
          parentId: undefined,
          extent: undefined,
          data: { id: 'b', label: 'B' },
        },
      ],
      edges: [
        {
          id: 'a-b-0',
          type: 'graphEdge',
          source: 'a',
          target: 'b',
          label: 'つながり',
          style: { strokeWidth: 1.5 },
          data: {
            source: 'a',
            target: 'b',
            label: 'つながり',
            dagre: {
              points: [
                { x: 160, y: 19 },
                { x: 235, y: 19 },
                { x: 310, y: 19 },
              ],
              labelPosition: { x: 235, y: 39 },
            },
          },
        },
      ],
    })
  })

  it('同じ node 間の edge に別々の Dagre 経路を割り当てる', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'a', label: 'A' },
        { id: 'b', label: 'B' },
      ],
      edges: [
        { source: 'a', target: 'b', label: 'relation one' },
        { source: 'a', target: 'b', label: 'relation two' },
      ],
    }
    const result = buildDagreLayout(def, 'LR', new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return

    expect(result.value.edges.map(({ data }) => data.dagre)).toEqual([
      {
        points: [
          { x: 160, y: 30.49425287356322 },
          { x: 254, y: 44 },
          { x: 348, y: 30.49425287356322 },
        ],
        labelPosition: { x: 254, y: 64 },
      },
      {
        points: [
          { x: 160, y: 7.505747126436782 },
          { x: 254, y: -6 },
          { x: 348, y: 7.505747126436782 },
        ],
        labelPosition: { x: 254, y: 14 },
      },
    ])
  })

  it('rankdir=TB では横並びではなく縦に積んで配置する', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'tree',
      nodes: [
        { id: 'a', label: 'A' },
        { id: 'b', label: 'B' },
      ],
      edges: [{ source: 'a', target: 'b' }],
    }
    const result = buildDagreLayout(def, 'TB', new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return
    expect(result.value.nodes.map((n) => n.position)).toEqual([
      { x: 0, y: 0 },
      { x: 0, y: 128 },
    ])
  })

  it('parent が循環参照していれば err を返す (throw させない)', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'a', label: 'A', parent: 'b' },
        { id: 'b', label: 'B', parent: 'a' },
      ],
      edges: [],
    }
    const result = buildDagreLayout(def, 'LR', new Map())
    expect(result.isErr()).toBe(true)
    if (!result.isErr()) return
    expect(result.error.message).toBe(
      'Setting a as parent of b would create a cycle',
    )
  })

  it('parent を持つノードは group にまとめられ、親が配列内で子より前に来る', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'child', label: 'Child', parent: 'group1' },
        { id: 'group1', label: 'Group 1' },
      ],
      edges: [],
    }
    const result = buildDagreLayout(def, 'LR', new Map())
    expect(result.isOk()).toBe(true)
    if (!result.isOk()) return

    expect(result.value).toEqual({
      nodes: [
        {
          id: 'group1',
          type: 'graphGroup',
          position: { x: 0, y: 0 },
          parentId: undefined,
          extent: undefined,
          data: { id: 'group1', label: 'Group 1' },
          style: { width: 250, height: 98 },
        },
        {
          id: 'child',
          type: 'graphNode',
          position: { x: 45, y: 30 },
          parentId: 'group1',
          extent: 'parent',
          data: { id: 'child', label: 'Child', parent: 'group1' },
        },
      ],
      edges: [],
    })
  })
})
