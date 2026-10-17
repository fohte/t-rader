import type { NodeProps } from '@xyflow/react'

import type { GraphFlowNode } from '#components/graph/flow-types'
import { getGraphHighlightClass } from '#components/graph/graph-highlight'
import { useGraphRenderContext } from '#components/graph/graph-render-context'

/** nodeTypes.graphGroup。破線ボーダーの箱 + 左上ラベル。サイズは dagre が計算した親ノードの style で決まる */
export function GraphGroupNodeView({ id, data }: NodeProps<GraphFlowNode>) {
  const { highlightedNodeIds } = useGraphRenderContext()
  const highlightClass = getGraphHighlightClass(id, highlightedNodeIds)

  return (
    <div
      className={`border-border bg-muted/20 relative h-full w-full rounded-md border border-dashed ${highlightClass}`}
    >
      <div className="text-muted-foreground absolute top-2 left-2.5 text-xs font-semibold tracking-wide">
        {data.label}
      </div>
    </div>
  )
}
