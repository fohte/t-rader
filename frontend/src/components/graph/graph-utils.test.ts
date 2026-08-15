import { ok } from 'neverthrow'
import { describe, expect, it } from 'vitest'

import {
  buildFlowEdges,
  computeCiteNumbers,
  edgeStrokeWidth,
  nodeWidth,
  validateGraphRefs,
} from '#components/graph/graph-utils'
import type { GraphDef, GraphNode } from '#components/graph/types'

describe('nodeWidth', () => {
  it.each([
    ['value なし', { id: 'a', label: 'A' } satisfies GraphNode, 160],
    ['value あり', { id: 'a', label: 'A', value: 60 } satisfies GraphNode, 208],
    [
      'value が 100 超で頭打ち',
      { id: 'a', label: 'A', value: 200 } satisfies GraphNode,
      240,
    ],
  ])('%s', (_label, node, expected) => {
    expect(nodeWidth(node)).toBe(expected)
  })
})

describe('edgeStrokeWidth', () => {
  it.each([
    ['value なし', undefined, 100, 1.5],
    ['maxValue が 0 以下', 50, 0, 1.5],
    ['value が maxValue と同じ (最大)', 100, 100, 5],
    ['value が maxValue の半分', 50, 100, 3.25],
  ])('%s', (_label, value, maxValue, expected) => {
    expect(edgeStrokeWidth(value, maxValue)).toBe(expected)
  })
})

describe('validateGraphRefs', () => {
  it('存在しない node id を参照していなければ ok を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'a', label: 'A' },
        { id: 'b', label: 'B', parent: 'a' },
      ],
      edges: [{ source: 'a', target: 'b' }],
    }
    expect(validateGraphRefs(def)).toEqual(ok(undefined))
  })

  it('node.parent が未知の id なら err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [{ id: 'a', label: 'A', parent: 'missing' }],
      edges: [],
    }
    const result = validateGraphRefs(def)
    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr().message).toBe(
      'graph "g1": node "a".parent = "missing" is not a known node id',
    )
  })

  it('edge.source が未知の id なら err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [{ id: 'a', label: 'A' }],
      edges: [{ source: 'missing', target: 'a' }],
    }
    const result = validateGraphRefs(def)
    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr().message).toBe(
      'graph "g1": edge.source = "missing" is not a known node id',
    )
  })

  it('edge.target が未知の id なら err を返す', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [{ id: 'a', label: 'A' }],
      edges: [{ source: 'a', target: 'missing' }],
    }
    const result = validateGraphRefs(def)
    expect(result.isErr()).toBe(true)
    expect(result._unsafeUnwrapErr().message).toBe(
      'graph "g1": edge.target = "missing" is not a known node id',
    )
  })
})

describe('computeCiteNumbers', () => {
  it('nodes → edges の出現順に 1 から連番を振り、同じ文字列は同じ番号にする', () => {
    const def: GraphDef = {
      id: 'g1',
      layout: 'flow',
      nodes: [
        { id: 'a', label: 'A', cite: '出典1' },
        { id: 'b', label: 'B', cite: '出典2' },
        { id: 'c', label: 'C' },
      ],
      edges: [
        { source: 'a', target: 'b', cite: '出典2' },
        { source: 'b', target: 'c', cite: '出典3' },
      ],
    }
    expect(computeCiteNumbers(def)).toEqual(
      new Map([
        ['出典1', 1],
        ['出典2', 2],
        ['出典3', 3],
      ]),
    )
  })
})

describe('buildFlowEdges', () => {
  it('label と cite からラベル文字列を組み立て、value から strokeWidth を計算する', () => {
    const citeNumbers = new Map([['出典1', 1]])
    const edges = [
      { source: 'a', target: 'b', label: '受託生産', value: 50 },
      { source: 'b', target: 'c', value: 100 },
      { source: 'c', target: 'd', label: '露光装置', cite: '出典1' },
      { source: 'd', target: 'e' },
    ]

    expect(buildFlowEdges(edges, citeNumbers)).toEqual([
      {
        id: 'a-b-0',
        source: 'a',
        target: 'b',
        label: '受託生産',
        style: { strokeWidth: 3.25 },
        data: edges[0],
      },
      {
        id: 'b-c-1',
        source: 'b',
        target: 'c',
        label: undefined,
        style: { strokeWidth: 5 },
        data: edges[1],
      },
      {
        id: 'c-d-2',
        source: 'c',
        target: 'd',
        label: '露光装置 [1]',
        style: { strokeWidth: 1.5 },
        data: edges[2],
      },
      {
        id: 'd-e-3',
        source: 'd',
        target: 'e',
        label: undefined,
        style: { strokeWidth: 1.5 },
        data: edges[3],
      },
    ])
  })
})
