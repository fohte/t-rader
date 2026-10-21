import {
  type TreemapDataNode,
  type TreemapRect,
} from '#components/widgets/widget-treemap-model'

export function isTreemapDataNode(value: unknown): value is TreemapDataNode {
  return (
    typeof value === 'object' &&
    value !== null &&
    'kind' in value &&
    (value.kind === 'group' || value.kind === 'row')
  )
}

export function isSameLayout(
  left: ReadonlyMap<string, TreemapRect>,
  right: ReadonlyMap<string, TreemapRect>,
): boolean {
  if (left.size !== right.size) return false
  for (const [key, rect] of right) {
    const previous = left.get(key)
    if (
      previous == null ||
      previous.x !== rect.x ||
      previous.y !== rect.y ||
      previous.width !== rect.width ||
      previous.height !== rect.height
    ) {
      return false
    }
  }
  return true
}
