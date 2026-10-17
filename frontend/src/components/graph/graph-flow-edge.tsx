import { BaseEdge, type EdgeProps, getBezierPath } from '@xyflow/react'

import type { GraphFlowEdge } from '#components/graph/flow-types'

export function GraphFlowEdgeView({
  data,
  id,
  interactionWidth,
  label,
  markerEnd,
  markerStart,
  sourcePosition,
  sourceX,
  sourceY,
  style,
  targetPosition,
  targetX,
  targetY,
}: EdgeProps<GraphFlowEdge>) {
  const [fallbackPath, fallbackLabelX, fallbackLabelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  })
  const route = data.dagre?.points
  const path =
    route != null && route.length > 1
      ? route
          .map((point, index) => {
            const command = index === 0 ? 'M' : 'L'
            return `${command} ${String(point.x)} ${String(point.y)}`
          })
          .join(' ')
      : fallbackPath
  const labelPosition = data.dagre?.labelPosition

  return (
    <BaseEdge
      id={id}
      path={path}
      style={style}
      markerStart={markerStart}
      markerEnd={markerEnd}
      interactionWidth={interactionWidth}
      label={label}
      labelX={labelPosition?.x ?? fallbackLabelX}
      labelY={labelPosition?.y ?? fallbackLabelY}
      labelShowBg={label != null}
      labelStyle={{ fill: 'var(--foreground)' }}
      labelBgStyle={{ fill: 'var(--card)' }}
    />
  )
}
