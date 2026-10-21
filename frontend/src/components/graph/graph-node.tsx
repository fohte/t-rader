import { Handle, type NodeProps } from '@xyflow/react'
import type { CSSProperties } from 'react'

import { CiteBadge } from '#components/graph/cite-badge'
import type { GraphFlowNode } from '#components/graph/flow-types'
import { useGraphRenderContext } from '#components/graph/graph-render-context'
import {
  handlePositions,
  nodeHeight,
  nodeWidth,
} from '#components/graph/graph-utils'
import { RefChip } from '#components/strategy-shell/ref-chip'
import { cn } from '#lib/utils'

const CHAIN_BAR_MAX_HEIGHT = 64
const CHAIN_BAR_MIN_HEIGHT = 4

/** nodeTypes.graphNode として登録する共通カスタムノード。layout の種類によらずこれ 1 つで描画を賄う */
export function GraphNodeView({ id, data }: NodeProps<GraphFlowNode>) {
  const { layout, maxNodeValue, citeNumbers, highlightedNodeIds, onOpenRef } =
    useGraphRenderContext()

  const handlePosition = handlePositions(layout)

  const citeNumber = data.cite != null ? citeNumbers.get(data.cite) : undefined

  const showBar =
    layout === 'chain' && typeof data.value === 'number' && maxNodeValue > 0
  const isHighlighted = highlightedNodeIds == null || highlightedNodeIds.has(id)
  const dagreHeight =
    layout === 'flow' || layout === 'tree' ? nodeHeight(data) : undefined
  // 値の大小を棒の高さで比較するため、計算結果を CSS 変数に渡す。
  const chainBarHeight = showBar
    ? Math.max(
        CHAIN_BAR_MIN_HEIGHT,
        ((data.value ?? 0) / maxNodeValue) * CHAIN_BAR_MAX_HEIGHT,
      )
    : undefined

  // graph-utils.ts の nodeHeight は、この要素の padding / gap / font size を見積もる
  // Dagre の配置と React Flow の測定が同じ寸法を使うよう、計算値を CSS 変数に渡す。
  return (
    <div
      data-slot="graph-node"
      className={cn(
        'border-border bg-card text-card-foreground relative flex w-(--graph-node-width) min-h-(--graph-node-min-height) flex-col gap-1.5 border px-3 py-2 text-sm',
        isHighlighted ? 'opacity-100' : 'opacity-30 transition-opacity',
      )}
      style={
        {
          '--graph-node-width': `${String(nodeWidth(data))}px`,
          '--graph-node-min-height':
            dagreHeight == null ? 'auto' : `${String(dagreHeight)}px`,
        } as CSSProperties & {
          '--graph-node-width': string
          '--graph-node-min-height': string
        }
      }
    >
      <Handle
        type="target"
        position={handlePosition.target}
        className="pointer-events-none opacity-0"
      />

      {citeNumber != null && data.cite != null && (
        <CiteBadge
          number={citeNumber}
          cite={data.cite}
          className="absolute -top-4 -right-4"
        />
      )}

      <div className="font-medium break-words">{data.label}</div>
      {data.ref != null && (
        <RefChip token={data.ref} pill onOpenRef={onOpenRef} />
      )}
      {chainBarHeight != null && (
        <div
          data-slot="graph-node-value-bar"
          className="bg-muted-foreground h-(--graph-node-bar-height) w-2"
          style={
            {
              '--graph-node-bar-height': `${String(chainBarHeight)}px`,
            } as CSSProperties & { '--graph-node-bar-height': string }
          }
        />
      )}

      <Handle
        type="source"
        position={handlePosition.source}
        className="pointer-events-none opacity-0"
      />
    </div>
  )
}
