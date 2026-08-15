import { Position } from '@xyflow/react'
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
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: { id: 'a', label: 'A' },
        },
        {
          id: 'b',
          type: 'graphNode',
          position: { x: 250, y: 0 },
          parentId: undefined,
          extent: undefined,
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: { id: 'b', label: 'B' },
        },
      ],
      edges: [
        {
          id: 'a-b-0',
          source: 'a',
          target: 'b',
          label: 'つながり',
          style: { strokeWidth: 1.5 },
          data: { source: 'a', target: 'b', label: 'つながり' },
        },
      ],
    })
  })

  it('rankdir=TB では source/target position が Bottom/Top になる', () => {
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
    expect(
      result.value.nodes.map((n) => ({
        id: n.id,
        sourcePosition: n.sourcePosition,
        targetPosition: n.targetPosition,
      })),
    ).toEqual([
      {
        id: 'a',
        sourcePosition: Position.Bottom,
        targetPosition: Position.Top,
      },
      {
        id: 'b',
        sourcePosition: Position.Bottom,
        targetPosition: Position.Top,
      },
    ])
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
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: { id: 'group1', label: 'Group 1' },
          style: { width: 250, height: 116 },
        },
        {
          id: 'child',
          type: 'graphNode',
          position: { x: 45, y: 30 },
          parentId: 'group1',
          extent: 'parent',
          sourcePosition: Position.Right,
          targetPosition: Position.Left,
          data: { id: 'child', label: 'Child', parent: 'group1' },
        },
      ],
      edges: [],
    })
  })
})
