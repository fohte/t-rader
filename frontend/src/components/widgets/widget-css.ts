import type { CSSProperties } from 'react'

export type WidgetCSSProperties = CSSProperties &
  Record<`--widget-${string}`, string>
