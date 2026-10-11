import {
  type AnnotationBandMarker,
  type AnnotationBandPeriod,
  groupAnnotationBandTargets,
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

export function AnnotationChartBand({
  markers,
  periods,
  width,
  bottom,
  selectedAnnotationId,
  onSelectAnnotation,
}: AnnotationChartBandProps) {
  const targetGroups = groupAnnotationBandTargets(markers, periods)
  const groups = [...targetGroups.periods, ...targetGroups.markers]

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
        const onlyPeriod = group.kind === 'period' && !hasMultipleTargets
        const centerX = group.centerX

        return (
          <div key={group.ids.join(':')} className="contents">
            {group.periods.map((period) => (
              <span
                key={period.id}
                aria-hidden="true"
                className={cn(
                  'pointer-events-none absolute z-0 top-1/2 h-1 -translate-y-1/2 rounded-full',
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
                  'pointer-events-none absolute z-20 top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-primary',
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
                onlyPeriod ? 'z-10' : 'z-20',
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
