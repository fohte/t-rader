import {
  type AnnotationBandMarker,
  clusterAnnotationBandMarkers,
} from '#lib/annotation-chart-utils'
import { cn } from '#lib/utils'

interface AnnotationChartBandProps {
  markers: AnnotationBandMarker[]
  width: number
  bottom: number
  selectedAnnotationId: string | null
  onSelectAnnotation?: (id: string) => void
}

export function AnnotationChartBand({
  markers,
  width,
  bottom,
  selectedAnnotationId,
  onSelectAnnotation,
}: AnnotationChartBandProps) {
  const clusters = clusterAnnotationBandMarkers(markers)

  return (
    <div
      aria-label="チャートのアノテーション"
      className="pointer-events-none absolute z-10 h-6 border-y border-border bg-card/90"
      role="group"
      style={{ bottom, left: 0, width }}
    >
      {clusters.map((cluster) => {
        const selectedIndex = cluster.markers.findIndex(
          (marker) => marker.id === selectedAnnotationId,
        )
        const isSelected = selectedIndex >= 0
        const nextMarker =
          cluster.markers[
            isSelected ? (selectedIndex + 1) % cluster.markers.length : 0
          ]
        if (nextMarker == null) return null

        return (
          <button
            key={cluster.markers.map((marker) => marker.id).join(':')}
            type="button"
            aria-label={
              cluster.markers.length === 1
                ? 'アノテーションを選択'
                : `アノテーション ${String(cluster.markers.length)} 件を順に選択`
            }
            aria-pressed={isSelected}
            className={cn(
              'pointer-events-auto absolute top-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center justify-center border border-primary transition-colors',
              cluster.markers.length === 1
                ? 'size-3 rounded-full'
                : 'h-5 min-w-5 rounded-full px-1 font-mono text-2xs',
              isSelected
                ? 'bg-primary text-primary-foreground ring-2 ring-primary/30'
                : 'bg-background text-primary hover:bg-primary/15',
            )}
            style={{ left: cluster.x }}
            title={
              cluster.markers.length === 1
                ? 'アノテーションを選択'
                : `${String(cluster.markers.length)} 件を順に選択`
            }
            onClick={() => {
              onSelectAnnotation?.(nextMarker.id)
            }}
          >
            {cluster.markers.length > 1 ? cluster.markers.length : null}
          </button>
        )
      })}
    </div>
  )
}
