// Rust の GraphDef/GraphNode/GraphEdge/Layout (backend) に対応する手書き型。
// 将来 openapi-typescript の生成型に差し替えられる前提の暫定実装。
//
// interface ではなく type を使うこと: React Flow の Node<T> は
// T extends Record<string, unknown> を要求する。type エイリアスの object literal は
// 暗黙の index signature を持つとみなされてこれを満たすが、interface は満たさない
// (TypeScript の既知の挙動)。

export type Layout = 'flow' | 'tree' | 'chain' | 'scatter'

export type GraphNode = {
  id: string
  label: string
  ref?: string
  value?: number
  cite?: string
  parent?: string
  x?: number
  y?: number
}

export type GraphEdge = {
  source: string
  target: string
  label?: string
  value?: number
  cite?: string
}

export type GraphDef = {
  id: string
  layout: Layout
  title?: string
  nodes: GraphNode[]
  edges: GraphEdge[]
}
