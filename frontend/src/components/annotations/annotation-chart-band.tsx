import {
  type AnnotationBandMarker,
  type AnnotationBandPeriod,
  clusterAnnotationBandMarkers,
} from '#lib/annotation-chart-utils'
import { cn } from '#lib/utils'

interface AnnotationChartBandProps {
  markers: AnnotationBandMarker[]
  periods: AnnotationBandPeriod[]
  width: number
  bottom: number
  selectedAnnotationId: string | null
  onSelectAnnotation?: (id: string) => void
}

interface BandTargetGroup {
  ids: string[]
  startX: number
  endX: number
  markers: AnnotationBandMarker[]
  periods: AnnotationBandPeriod[]
}

function groupBandTargets(
  markers: AnnotationBandMarker[],
  periods: AnnotationBandPeriod[],
): BandTargetGroup[] {
  const targets: BandTargetGroup[] = [
    ...clusterAnnotationBandMarkers(markers).map((cluster) => ({
      ids: cluster.markers.map((marker) => marker.id),
      startX: cluster.markers[0]?.x ?? cluster.x,
      endX: cluster.markers.at(-1)?.x ?? cluster.x,
      markers: cluster.markers,
      periods: [],
    })),
    ...periods.map((period) => ({
      ids: [period.id],
      startX: period.startX,
      endX: period.endX,
      markers: [],
      periods: [period],
    })),
  ].sort((left, right) => left.startX - right.startX)
  const groups: BandTargetGroup[] = []

  for (const target of targets) {
    const current = groups.at(-1)
    if (current != null && target.startX - current.endX < 16) {
      current.ids.push(...target.ids)
      current.endX = Math.max(current.endX, target.endX)
      current.markers.push(...target.markers)
      current.periods.push(...target.periods)
    } else {
      groups.push({ ...target })
    }
  }

  return groups
}

export function AnnotationChartBand({
  markers,
  periods,
  width,
  bottom,
  selectedAnnotationId,
  onSelectAnnotation,
}: AnnotationChartBandProps) {
  const groups = groupBandTargets(markers, periods)

  return (
    <div
      aria-label="チャートのアノテーション"
      className="pointer-events-none absolute z-10 h-6 border-y border-border bg-card/90"
      role="group"
      style={{ bottom, left: 0, width }}
    >
      {groups.map((group) => {
        const selectedIndex = group.ids.findIndex(
          (id) => id === selectedAnnotationId,
        )
        const isSelected = selectedIndex >= 0
        const nextId =
          group.ids[isSelected ? (selectedIndex + 1) % group.ids.length : 0]
        if (nextId == null) return null

        const hasMultipleTargets = group.ids.length > 1
        const onlyPeriod = group.periods.length === 1 && !hasMultipleTargets
        const centerX = (group.startX + group.endX) / 2

        return (
          <div key={group.ids.join(':')} className="contents">
            {group.periods.map((period) => (
              <span
                key={period.id}
                aria-hidden="true"
                className={cn(
                  'pointer-events-none absolute top-1/2 h-1 -translate-y-1/2 rounded-full',
                  period.id === selectedAnnotationId
                    ? 'bg-primary'
                    : 'bg-primary/55',
                )}
                style={{
                  left: period.startX,
                  width: Math.max(period.endX - period.startX, 1),
                }}
              />
            ))}
            {group.markers.map((marker) => (
              <span
                key={marker.id}
                aria-hidden="true"
                className={cn(
                  'pointer-events-none absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-primary',
                  marker.id === selectedAnnotationId
                    ? 'bg-primary'
                    : 'bg-background',
                )}
                style={{ left: marker.x }}
              />
            ))}
            <button
              type="button"
              aria-label={
                hasMultipleTargets
                  ? `アノテーション ${String(group.ids.length)} 件を順に選択`
                  : onlyPeriod
                    ? '期間アノテーションを選択'
                    : 'アノテーションを選択'
              }
              aria-pressed={isSelected}
              className={cn(
                'pointer-events-auto absolute top-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center justify-center border border-primary transition-colors',
                hasMultipleTargets
                  ? 'h-5 min-w-5 rounded-full px-1 font-mono text-2xs'
                  : onlyPeriod
                    ? 'h-2 rounded-full'
                    : 'size-3 rounded-full',
                isSelected
                  ? 'bg-primary text-primary-foreground ring-2 ring-primary/30'
                  : 'bg-background text-primary hover:bg-primary/15',
              )}
              style={
                onlyPeriod
                  ? {
                      left: centerX,
                      width: Math.max(group.endX - group.startX, 8),
                    }
                  : { left: centerX }
              }
              title={
                hasMultipleTargets
                  ? `${String(group.ids.length)} 件を順に選択`
                  : onlyPeriod
                    ? '期間アノテーションを選択'
                    : 'アノテーションを選択'
              }
              onClick={() => {
                onSelectAnnotation?.(nextId)
              }}
            >
              {hasMultipleTargets ? group.ids.length : null}
            </button>
          </div>
        )
      })}
    </div>
  )
}
