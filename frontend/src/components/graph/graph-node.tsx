import { Handle, type NodeProps } from '@xyflow/react'

import { CiteBadge } from '#components/graph/cite-badge'
import type { GraphFlowNode } from '#components/graph/flow-types'
import { useGraphRenderContext } from '#components/graph/graph-render-context'
import { handlePositions, nodeWidth } from '#components/graph/graph-utils'
import { RefChip } from '#components/strategy-shell/ref-chip'

const CHAIN_BAR_MAX_HEIGHT = 64
const CHAIN_BAR_MIN_HEIGHT = 4

/** nodeTypes.graphNode として登録する共通カスタムノード。layout の種類によらずこれ 1 つで描画を賄う */
export function GraphNodeView({ data }: NodeProps<GraphFlowNode>) {
  const { layout, maxNodeValue, citeNumbers, onOpenRef } =
    useGraphRenderContext()

  const handlePosition = handlePositions(layout)

  const citeNumber = data.cite != null ? citeNumbers.get(data.cite) : undefined

  const showBar =
    layout === 'chain' && typeof data.value === 'number' && maxNodeValue > 0

  return (
    <div
      data-slot="graph-node"
      className="border-border bg-card text-card-foreground relative flex flex-col gap-1.5 rounded-md border px-3 py-2 text-sm"
      style={{ width: nodeWidth(data) }}
    >
      <Handle
        type="target"
        position={handlePosition.target}
        style={{ opacity: 0, pointerEvents: 'none' }}
      />

      {citeNumber != null && data.cite != null && (
        <CiteBadge
          number={citeNumber}
          cite={data.cite}
          className="absolute -top-2 -right-2"
        />
      )}

      <div className="font-medium">{data.label}</div>
      {data.ref != null && <RefChip token={data.ref} pill onOpen={onOpenRef} />}
      {showBar && typeof data.value === 'number' && (
        <div
          className="bg-primary"
          style={{
            width: 8,
            height: Math.max(
              CHAIN_BAR_MIN_HEIGHT,
              (data.value / maxNodeValue) * CHAIN_BAR_MAX_HEIGHT,
            ),
          }}
        />
      )}

      <Handle
        type="source"
        position={handlePosition.source}
        style={{ opacity: 0, pointerEvents: 'none' }}
      />
    </div>
  )
}
