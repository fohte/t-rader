import { IndicatorObservationChart } from '#components/indicator-observation-chart'
import { RelatedNotes } from '#components/related-notes'
import type { components } from '#lib/api/schema.gen'
import {
  getIndicatorObservationSummary,
  type IndicatorPeriod,
  selectIndicatorObservations,
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
      observations:
        components['schemas']['IndicatorObservationResponse'][] | null
      isObservationsLoading: boolean
      isObservationsError: boolean
      notes: Note[] | null
      isNotesError: boolean
      period: IndicatorPeriod
      onPeriodChange: (period: IndicatorPeriod) => void
    }

export function IndicatorDetailPage(props: IndicatorDetailPageProps) {
  if (props.state === 'loading') {
    return (
      <main className="space-y-5 font-sans text-foreground" aria-busy="true">
        <div className="space-y-3">
          <div className="h-8 w-48 animate-pulse bg-surface-strong" />
          <div className="h-9 w-36 animate-pulse bg-surface-strong" />
        </div>
        <p className="font-mono text-xs text-muted-foreground">
          指標を読み込んでいます: {props.indicatorId}
        </p>
      </main>
    )
  }

  if (props.state === 'not-found') {
    return (
      <main className="space-y-2 font-sans text-foreground">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          指標が見つかりません
        </h1>
        <p className="font-mono text-sm text-muted-foreground">
          {props.indicatorId}
        </p>
      </main>
    )
  }

  if (props.state === 'error') {
    return (
      <main className="space-y-2 font-sans text-foreground">
        <h1 className="text-2xl font-bold leading-tight tracking-tight">
          指標を読み込めませんでした
        </h1>
        <p className="font-mono text-sm text-muted-foreground">
          {props.indicatorId}
        </p>
      </main>
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
                  className={
                    observationSummary.change != null &&
                    observationSummary.change < 0
                      ? 'font-mono text-sm tabular-nums text-destructive'
                      : 'font-mono text-sm tabular-nums text-emerald-600 dark:text-emerald-400'
                  }
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
            observations={selectIndicatorObservations(
              props.observations ?? [],
              props.period,
            )}
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
