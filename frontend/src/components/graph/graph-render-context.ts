import { createContext, useContext } from 'react'

import type { Layout } from '#components/graph/types'

// React Flow の Node<T> の data は GraphNode そのものに固定する設計 (GraphNode 以外を
// 入れたら型エラーにしたい)。hover 状態や cite 番号など描画専用の情報を data に混ぜず、
// Context 経由でカスタムノード/背景に渡す。
export interface GraphRenderContextValue {
  layout: Layout
  maxNodeValue: number
  citeNumbers: Map<string, number>
  onOpenRef?: (token: string) => void
}

const GraphRenderContext = createContext<GraphRenderContextValue | null>(null)
export const GraphRenderContextProvider = GraphRenderContext.Provider

export function useGraphRenderContext(): GraphRenderContextValue {
  const value = useContext(GraphRenderContext)
  if (value == null) {
    // eslint-disable-next-line no-restricted-syntax -- React hook の契約上 Result を返せない。呼び出し側は直接分割代入する
    throw new Error('useGraphRenderContext must be used within GraphRenderer')
  }
  return value
}
