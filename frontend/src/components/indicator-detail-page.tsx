import { IndicatorObservationChart } from '#components/indicator-observation-chart'
import { ReferenceDetailState } from '#components/reference-detail-state'
import { RelatedNotes } from '#components/related-notes'
import { optionalPnlColorClass } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'
import {
  getIndicatorObservationSummary,
  type IndicatorObservation,
  type IndicatorPeriod,
} from '#lib/indicator-observation'

type Indicator = components['schemas']['Indicator']
type Note = components['schemas']['Note']

type IndicatorDetailPageProps =
  | { state: 'loading'; indicatorId: string }
  | { state: 'not-found'; indicatorId: string }
  | { state: 'error'; indicatorId: string }
  | {
      state: 'loaded'
      indicator: Indicator
      observations: IndicatorObservation[] | null
      visibleObservations: IndicatorObservation[]
      isObservationsLoading: boolean
      isObservationsError: boolean
      notes: Note[] | null
      isNotesError: boolean
      period: IndicatorPeriod
      onPeriodChange: (period: IndicatorPeriod) => void
    }

export function IndicatorDetailPage(props: IndicatorDetailPageProps) {
  if (props.state !== 'loaded') {
    return (
      <ReferenceDetailState
        state={props.state}
        label="指標"
        identifier={props.indicatorId}
      />
    )
  }

  const observationSummary =
    props.observations == null || props.isObservationsError
      ? null
      : getIndicatorObservationSummary(props.observations)
  const changeLabel =
    observationSummary?.change == null
      ? null
      : `${formatSignedValue(observationSummary.change)}${
          observationSummary.changePercent == null
            ? ''
            : ` (${formatSignedPercent(observationSummary.changePercent)}%)`
        }`

  return (
    <main className="space-y-5 font-sans text-foreground">
      <header className="space-y-2">
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <h1 className="text-2xl font-bold leading-tight tracking-tight">
            {props.indicator.name}
          </h1>
          <span className="font-mono text-xs text-muted-foreground">
            {props.indicator.id}
          </span>
        </div>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          {props.isObservationsLoading ? (
            <div className="h-9 w-32 animate-pulse bg-surface-strong" />
          ) : observationSummary != null ? (
            <>
              <span className="font-mono text-3xl font-semibold tabular-nums">
                {formatValue(observationSummary.latest.value)}
              </span>
              {changeLabel != null && (
                <span
                  className={`font-mono text-sm tabular-nums ${optionalPnlColorClass(observationSummary.change)}`}
                >
                  {changeLabel}
                </span>
              )}
              <span className="font-mono text-xs text-muted-foreground">
                {observationSummary.latest.date} 時点
              </span>
            </>
          ) : (
            <span className="text-sm text-muted-foreground">
              {props.isObservationsError
                ? '最新値を読み込めませんでした。'
                : '観測値はありません。'}
            </span>
          )}
        </div>
      </header>

      <div className="grid grid-cols-1 items-start gap-5 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <IndicatorObservationChart
            observations={props.visibleObservations}
            period={props.period}
            onPeriodChange={props.onPeriodChange}
            isPending={props.isObservationsLoading}
            isError={props.isObservationsError}
          />
        </div>
        <div className="lg:col-span-1">
          <RelatedNotes notes={props.notes} isError={props.isNotesError} />
        </div>
      </div>
    </main>
  )
}

function formatValue(value: number): string {
  return new Intl.NumberFormat('ja-JP', {
    maximumSignificantDigits: 8,
  }).format(value)
}

function formatSignedValue(value: number): string {
  const sign = value >= 0 ? '+' : ''
  return `${sign}${formatValue(value)}`
}

function formatSignedPercent(value: number): string {
  const sign = value >= 0 ? '+' : ''
  return `${sign}${new Intl.NumberFormat('ja-JP', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value)}`
}
