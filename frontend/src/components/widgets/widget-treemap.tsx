import { Link } from '@tanstack/react-router'
import { hierarchy, treemap } from 'd3-hierarchy'

import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetRow = components['schemas']['WidgetQueryResultRow']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

type TreemapLeaf = { row: WidgetRow; weight: number; color: number }
type TreemapData = TreemapLeaf | { children: TreemapLeaf[] }

const LAYOUT_WIDTH = 1200
const LAYOUT_HEIGHT = 675
const numberFormat = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

function groupTarget(key: string) {
  if (!key.startsWith('group:')) return null

  const groupRef = key.slice('group:'.length)
  const separator = groupRef.indexOf('/')
  if (separator <= 0 || separator === groupRef.length - 1) return null

  return {
    axisKey: groupRef.slice(0, separator),
    groupKey: groupRef.slice(separator + 1),
  }
}

export function WidgetTreemap({
  result,
  viz,
}: {
  result: WidgetQueryResponse
  viz: WidgetTreemapDefinition
}) {
  const leaves = result.rows.flatMap((row): TreemapLeaf[] => {
    const sizeValue = row.values[viz.size]
    const colorValue = row.values[viz.color]
    if (
      typeof sizeValue !== 'number' ||
      !Number.isFinite(sizeValue) ||
      typeof colorValue !== 'number' ||
      !Number.isFinite(colorValue)
    ) {
      return []
    }

    const weight = Math.abs(sizeValue)
    return weight === 0 ? [] : [{ row, weight, color: colorValue }]
  })

  const root = hierarchy<TreemapData>({ children: leaves }, (node) =>
    'children' in node ? node.children : undefined,
  )
    .sum((node) => ('weight' in node ? node.weight : 0))
    .sort((left, right) => {
      const valueOrder = (right.value ?? 0) - (left.value ?? 0)
      if (valueOrder !== 0) return valueOrder
      const leftKey = 'row' in left.data ? left.data.row.key : ''
      const rightKey = 'row' in right.data ? right.data.row.key : ''
      return leftKey.localeCompare(rightKey)
    })

  const tiles = treemap<TreemapData>()
    .size([LAYOUT_WIDTH, LAYOUT_HEIGHT])
    .paddingInner(2)(root)
    .leaves()
    .flatMap((node) => {
      if (!('row' in node.data)) return []
      const width = node.x1 - node.x0
      const height = node.y1 - node.y0
      if (width <= 0 || height <= 0) return []
      return [{ ...node.data, x: node.x0, y: node.y0, width, height }]
    })

  if (tiles.length === 0) {
    return (
      <p
        className="border border-border bg-card px-4 py-8 text-center font-mono text-xs text-muted-foreground"
        role="status"
      >
        表示できるデータがありません。
      </p>
    )
  }

  return (
    <ul
      aria-label="ウィジェットの treemap"
      className="relative aspect-video w-full list-none overflow-hidden border border-border bg-card"
    >
      {tiles.map((tile) => {
        const colorStyle =
          tile.color > 0
            ? {
                backgroundColor: 'var(--color-up-dim)',
                borderColor: 'var(--color-up)',
              }
            : tile.color < 0
              ? {
                  backgroundColor: 'var(--color-down-dim)',
                  borderColor: 'var(--color-down)',
                }
              : {
                  backgroundColor: 'var(--color-bg-tertiary)',
                  borderColor: 'var(--color-border)',
                }
        const label = `${tile.row.name}: ${numberFormat.format(tile.color)}`
        const position = {
          left: `${String((tile.x / LAYOUT_WIDTH) * 100)}%`,
          top: `${String((tile.y / LAYOUT_HEIGHT) * 100)}%`,
          width: `${String((tile.width / LAYOUT_WIDTH) * 100)}%`,
          height: `${String((tile.height / LAYOUT_HEIGHT) * 100)}%`,
          ...colorStyle,
        }
        const contents = (
          <>
            {tile.width >= LAYOUT_WIDTH / 3 &&
              tile.height >= LAYOUT_HEIGHT / 3 && (
                <>
                  <span className="line-clamp-1 font-sans text-xs font-semibold text-foreground">
                    {tile.row.name}
                  </span>
                  <span className="mt-auto font-mono text-2xs tabular-nums text-foreground/80">
                    {numberFormat.format(tile.color)}
                  </span>
                </>
              )}
          </>
        )
        const target = groupTarget(tile.row.key)

        return (
          <li
            aria-label={label}
            className="absolute overflow-hidden border p-1.5"
            key={tile.row.key}
            style={position}
            title={label}
          >
            {target == null ? (
              <div className="flex h-full w-full flex-col overflow-hidden">
                {contents}
              </div>
            ) : (
              <Link
                aria-label={label}
                className="flex h-full w-full flex-col overflow-hidden transition-opacity hover:opacity-90 focus-visible:outline-2 focus-visible:outline-ring"
                params={target}
                to="/groups/$axisKey/$groupKey"
              >
                {contents}
              </Link>
            )}
          </li>
        )
      })}
    </ul>
  )
}
