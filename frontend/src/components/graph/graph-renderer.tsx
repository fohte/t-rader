import '@xyflow/react/dist/style.css'

import {
  Background,
  type NodeTypes,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
} from '@xyflow/react'
import { useEffect, useMemo, useState } from 'react'

import { buildDagreLayout } from '#components/graph/dagre-layout'
import type {
  GraphFlowEdge,
  GraphFlowNode,
  ScatterBackgroundFlowNode,
} from '#components/graph/flow-types'
import { GraphGroupNodeView } from '#components/graph/graph-group-node'
import { GraphNodeView } from '#components/graph/graph-node'
import { GraphRenderContextProvider } from '#components/graph/graph-render-context'
import { GraphScatterBackgroundView } from '#components/graph/graph-scatter-background'
import { computeCiteNumbers } from '#components/graph/graph-utils'
import {
  buildChainLayout,
  buildScatterLayout,
} from '#components/graph/simple-layouts'
import type { GraphDef } from '#components/graph/types'

const nodeTypes: NodeTypes = {
  graphNode: GraphNodeView,
  graphGroup: GraphGroupNodeView,
  graphScatterBackground: GraphScatterBackgroundView,
}

const HIGHLIGHT_CLASS = 'opacity-100'
const DIMMED_CLASS = 'opacity-30 transition-opacity'

export interface GraphRendererProps {
  def: GraphDef
  onOpenRef?: (token: string) => void
  className?: string
}

export function GraphRenderer({
  def,
  onOpenRef,
  className,
}: GraphRendererProps) {
  return (
    <div className={className}>
      <ReactFlowProvider>
        <GraphCanvas def={def} onOpenRef={onOpenRef} />
      </ReactFlowProvider>
    </div>
  )
}

interface GraphCanvasProps {
  def: GraphDef
  onOpenRef?: (token: string) => void
}

function GraphCanvas({ def, onOpenRef }: GraphCanvasProps) {
  const { fitView } = useReactFlow()
  const [hoveredId, setHoveredId] = useState<string | null>(null)

  const citeNumbers = useMemo(() => computeCiteNumbers(def), [def])
  const maxNodeValue = useMemo(
    () => Math.max(0, ...def.nodes.map((n) => n.value ?? 0)),
    [def.nodes],
  )

  const layoutResult = useMemo(() => {
    switch (def.layout) {
      case 'flow':
        return buildDagreLayout(def, 'LR', citeNumbers)
      case 'tree':
        return buildDagreLayout(def, 'TB', citeNumbers)
      case 'chain':
        return buildChainLayout(def, citeNumbers)
      case 'scatter':
        return buildScatterLayout(def, citeNumbers)
    }
  }, [def, citeNumbers])

  // hooks はすべて呼び終えてから条件分岐する (Rules of Hooks)。エラー時は空配列で埋める
  const rawNodes: (GraphFlowNode | ScatterBackgroundFlowNode)[] =
    layoutResult.isOk() ? layoutResult.value.nodes : []
  const rawEdges: GraphFlowEdge[] = layoutResult.isOk()
    ? layoutResult.value.edges
    : []

  // hoveredId から 1 hop で繋がるノード id の集合。元の GraphEdge (座標を持たない) を辿る
  const neighborIds = useMemo(() => {
    if (hoveredId == null) return null
    const ids = new Set([hoveredId])
    for (const edge of def.edges) {
      if (edge.source === hoveredId) ids.add(edge.target)
      if (edge.target === hoveredId) ids.add(edge.source)
    }
    return ids
  }, [hoveredId, def.edges])

  const nodes = useMemo(
    () =>
      rawNodes.map((node) => ({
        ...node,
        className:
          neighborIds == null || neighborIds.has(node.id)
            ? HIGHLIGHT_CLASS
            : DIMMED_CLASS,
      })),
    [rawNodes, neighborIds],
  )
  const edges = useMemo(
    () =>
      rawEdges.map((edge) => ({
        ...edge,
        className:
          neighborIds == null ||
          edge.source === hoveredId ||
          edge.target === hoveredId
            ? HIGHLIGHT_CLASS
            : DIMMED_CLASS,
      })),
    [rawEdges, neighborIds, hoveredId],
  )

  // グラフ形状 (layout 結果) が変わるたびに、見える範囲を追従させる。hover によるハイライト
  // 変更では再フィットしないよう、装飾前の rawNodes/rawEdges を依存に使う
  useEffect(() => {
    void fitView({ duration: 200, padding: 0.2 })
  }, [rawNodes, rawEdges, fitView])

  if (layoutResult.isErr()) {
    return (
      <div role="alert" className="text-destructive p-4 text-sm">
        {layoutResult.error.message}
      </div>
    )
  }

  return (
    <GraphRenderContextProvider
      value={{ layout: def.layout, maxNodeValue, citeNumbers, onOpenRef }}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={false}
        fitView
        onNodeMouseEnter={(_, node) => {
          setHoveredId(node.id)
        }}
        onNodeMouseLeave={() => {
          setHoveredId(null)
        }}
      >
        <Background />
      </ReactFlow>
    </GraphRenderContextProvider>
  )
}
