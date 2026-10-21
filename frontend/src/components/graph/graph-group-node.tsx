import type { NodeProps } from '@xyflow/react'

import type { GraphFlowNode } from '#components/graph/flow-types'
import { useGraphRenderContext } from '#components/graph/graph-render-context'
import { cn } from '#lib/utils'

/** nodeTypes.graphGroup。破線ボーダーの箱 + 左上ラベル。サイズは dagre が計算した親ノードの style で決まる */
export function GraphGroupNodeView({ id, data }: NodeProps<GraphFlowNode>) {
  const { highlightedNodeIds } = useGraphRenderContext()
  const isHighlighted = highlightedNodeIds == null || highlightedNodeIds.has(id)

  return (
    <div
      className={cn(
        'border-border bg-muted/20 relative h-full w-full border border-dashed',
        isHighlighted ? 'opacity-100' : 'opacity-30 transition-opacity',
      )}
    >
      <div className="text-muted-foreground absolute top-2 left-2.5 text-xs font-semibold tracking-wide">
        {data.label}
      </div>
    </div>
  )
}
