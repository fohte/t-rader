import { Button } from '@fohte/ui/button'
import type { CSSProperties } from 'react'

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
  const groups = groupAnnotationBandTargets(markers, periods)
  const bandStyle: CSSProperties & {
    '--annotation-band-bottom': string
    '--annotation-band-width': string
  } = {
    '--annotation-band-bottom': `${bottom.toString()}px`,
    '--annotation-band-width': `${width.toString()}px`,
  }

  return (
    <div
      aria-label="チャートのアノテーション"
      className="pointer-events-none absolute bottom-(--annotation-band-bottom) left-0 z-10 h-6 w-(--annotation-band-width) border-y border-border bg-card/90"
      role="group"
      style={bandStyle}
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
        const targetStyle: CSSProperties & {
          '--annotation-target-left': string
        } = {
          '--annotation-target-left': `${centerX.toString()}px`,
        }
        const targetContentStyle:
          | (CSSProperties & {
              '--annotation-target-width': string
            })
          | undefined = onlyPeriod
          ? {
              '--annotation-target-width': `${Math.max(group.endX - group.startX, 8).toString()}px`,
            }
          : undefined

        return (
          <div key={group.ids.join(':')} className="contents">
            {group.kind === 'period' &&
              group.periods.map((period) => {
                const periodStyle: CSSProperties & {
                  '--annotation-period-left': string
                  '--annotation-period-width': string
                } = {
                  '--annotation-period-left': `${period.startX.toString()}px`,
                  '--annotation-period-width': `${Math.max(period.endX - period.startX, 1).toString()}px`,
                }

                return (
                  <span
                    key={period.id}
                    aria-hidden="true"
                    className={cn(
                      'pointer-events-none absolute left-(--annotation-period-left) top-1/2 z-0 h-1 w-(--annotation-period-width) -translate-y-1/2 rounded-full',
                      period.id === selectedAnnotationId
                        ? 'bg-muted-foreground'
                        : 'bg-muted-foreground/55',
                    )}
                    style={periodStyle}
                  />
                )
              })}
            {group.kind === 'marker' &&
              group.markers.map((marker) => {
                const markerStyle: CSSProperties & {
                  '--annotation-marker-left': string
                } = {
                  '--annotation-marker-left': `${marker.x.toString()}px`,
                }

                return (
                  <span
                    key={marker.id}
                    aria-hidden="true"
                    className={cn(
                      'pointer-events-none absolute left-(--annotation-marker-left) top-1/2 z-20 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border border-border-strong',
                      marker.id === selectedAnnotationId
                        ? 'border-muted-foreground bg-muted-foreground'
                        : 'bg-background',
                    )}
                    style={markerStyle}
                  />
                )
              })}
            <Button
              variant="plain"
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
                'pointer-events-auto absolute left-(--annotation-target-left) top-1/2 -translate-x-1/2 -translate-y-1/2',
                onlyPeriod ? 'z-10' : 'z-20',
              )}
              style={targetStyle}
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
              <span
                className={cn(
                  'flex items-center justify-center border transition-colors',
                  hasMultipleTargets
                    ? 'h-5 min-w-5 rounded-full px-1 font-mono text-2xs'
                    : onlyPeriod
                      ? 'h-2 w-(--annotation-target-width) rounded-full'
                      : 'size-3 rounded-full',
                  isSelected
                    ? 'border-muted-foreground bg-muted-foreground text-background'
                    : 'border-border-strong bg-background text-foreground group-hover/button:bg-muted',
                )}
                style={targetContentStyle}
              >
                {hasMultipleTargets ? group.ids.length : null}
              </span>
            </Button>
          </div>
        )
      })}
    </div>
  )
}
