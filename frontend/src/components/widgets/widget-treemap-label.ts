export interface TreemapRect {
  x: number
  y: number
  width: number
  height: number
}

export type TreemapLabelMode = 'both' | 'value' | null

const LABEL_HORIZONTAL_PADDING = 8
const NARROW_GLYPH_WIDTH = 7
const WIDE_GLYPH_WIDTH = 12

function estimateLabelWidth(text: string): number {
  return Array.from(text).reduce(
    (width, character) =>
      width +
      (/^[\u0000-\u00ff]$/u.test(character)
        ? NARROW_GLYPH_WIDTH
        : WIDE_GLYPH_WIDTH),
    0,
  )
}

export function getTreemapLabelMode(
  rect: TreemapRect,
  name: string,
  value: string,
): TreemapLabelMode {
  const availableWidth = rect.width - LABEL_HORIZONTAL_PADDING
  const availableHeight = rect.height - LABEL_HORIZONTAL_PADDING
  if (availableWidth < estimateLabelWidth(value)) return null
  if (availableHeight >= 32 && availableWidth >= estimateLabelWidth(name)) {
    return 'both'
  }
  if (availableHeight >= 18) return 'value'
  return null
}
