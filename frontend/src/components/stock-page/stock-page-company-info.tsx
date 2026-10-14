import { StockPageHoldings } from '#components/stock-page/stock-page-holdings'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import {
  formatDecimal,
  formatMillionYenInHundredMillions,
  formatRatioAsPercent,
} from '#lib/stock-page-format'
import type { StockPageListState } from '#lib/stock-page-model'

type Position = components['schemas']['PositionSummary']
type NextEarnings = components['schemas']['StockNextEarningsResponse']
type Valuation = components['schemas']['StockValuationResponse']

const dateFormatter = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  weekday: 'short',
  timeZone: 'Asia/Tokyo',
})

function formatDate(date: string): string {
  return dateFormatter.format(new Date(`${date}T00:00:00+09:00`))
}

function selectPer(valuation: Valuation | null) {
  return {
    label: valuation?.fwd_per == null ? 'PER' : 'PER (予)',
    value: valuation?.fwd_per ?? valuation?.per,
  }
}

function selectRoe(valuation: Valuation | null) {
  return {
    label: valuation?.fwd_roe == null ? 'ROE' : 'ROE (予)',
    value: valuation?.fwd_roe ?? valuation?.roe,
  }
}

function SectionHeading({ title, detail }: { title: string; detail?: string }) {
  return (
    <div className="flex items-center justify-between gap-2 border-b border-border px-3.5 py-2">
      <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
        <span className="mr-2 text-primary">&gt;</span>
        {title}
      </h2>
      {detail != null && (
        <span className="font-mono text-2xs text-muted-foreground">
          {detail}
        </span>
      )}
    </div>
  )
}

function Metric({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail?: string
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1 px-3 py-2.5">
      <dt className="font-mono text-2xs text-muted-foreground">{label}</dt>
      <dd className="flex min-w-0 flex-col truncate font-mono text-sm tabular-nums text-foreground">
        {value}
        {detail != null && (
          <span className="truncate font-mono text-2xs text-muted-foreground">
            {detail}
          </span>
        )}
      </dd>
    </div>
  )
}

function QueryMessage({
  state,
  error,
  empty,
}: {
  state: StockPageListState
  error: string
  empty: string
}) {
  if (state === 'loading') return <Skeleton className="h-16 w-full" />

  return (
    <p
      className={`px-3.5 py-3 font-mono text-xs ${state === 'error' ? 'text-destructive' : 'text-muted-foreground'}`}
    >
      {state === 'error' ? error : empty}
    </p>
  )
}

function NextEarningsCard({
  earnings,
  state,
}: {
  earnings: NextEarnings | null
  state: StockPageListState
}) {
  return (
    <section className="border border-border bg-card" aria-label="次の決算">
      <SectionHeading title="次の決算" />
      {state !== 'ready' ? (
        <QueryMessage
          state={state}
          error="次の決算の取得に失敗しました"
          empty="決算予定はありません"
        />
      ) : earnings == null ? (
        <QueryMessage state="ready" error="" empty="決算予定はありません" />
      ) : (
        <div className="space-y-1 px-3.5 py-3">
          <time
            dateTime={earnings.event_date}
            className="font-mono text-sm font-bold tabular-nums"
          >
            {formatDate(earnings.event_date)}
          </time>
          <p className="text-xs text-foreground">{earnings.title}</p>
          {(earnings.fiscal_period != null || earnings.time_of_day != null) && (
            <p className="font-mono text-2xs text-muted-foreground">
              {[earnings.fiscal_period, earnings.time_of_day]
                .filter((value) => value != null)
                .join(' · ')}
            </p>
          )}
        </div>
      )}
    </section>
  )
}

function ValuationCard({
  valuation,
  state,
}: {
  valuation: Valuation | null
  state: StockPageListState
}) {
  const per = selectPer(valuation)
  const roe = selectRoe(valuation)

  return (
    <section
      className="border border-border bg-card"
      aria-label="バリュエーション"
    >
      <SectionHeading
        title="バリュエーション"
        detail={valuation == null ? undefined : `${valuation.date} 時点`}
      />
      {state !== 'ready' ? (
        <QueryMessage
          state={state}
          error="バリュエーションの取得に失敗しました"
          empty="バリュエーションはありません"
        />
      ) : valuation == null ? (
        <QueryMessage
          state="ready"
          error=""
          empty="バリュエーションはありません"
        />
      ) : (
        <dl className="grid grid-cols-2 divide-x divide-y divide-border">
          <Metric label={per.label} value={formatDecimal(per.value)} />
          <Metric label="PBR" value={formatDecimal(valuation.pbr)} />
          <Metric label={roe.label} value={formatRatioAsPercent(roe.value)} />
          <Metric
            label="時価総額"
            value={formatMillionYenInHundredMillions(valuation.mkt_cap)}
          />
        </dl>
      )}
    </section>
  )
}

function MobileOverview({
  position,
  holdingsState,
  earnings,
  earningsState,
  valuation,
  valuationState,
}: {
  position: Position | null
  holdingsState: StockPageListState
  earnings: NextEarnings | null
  earningsState: StockPageListState
  valuation: Valuation | null
  valuationState: StockPageListState
}) {
  const per = selectPer(valuation)
  const holdingValue = formatStateValue(
    holdingsState,
    position,
    '保有なし',
    (value) => `${value.qty.toLocaleString('ja-JP')} 株`,
  )
  const earningsValue = formatStateValue(
    earningsState,
    earnings,
    '予定なし',
    (value) => formatDate(value.event_date),
  )
  const valuationValue = formatStateValue(
    valuationState,
    valuation,
    'データなし',
    () => formatDecimal(per.value),
  )
  const pbrValue = formatStateValue(
    valuationState,
    valuation,
    'データなし',
    (value) => formatDecimal(value.pbr),
  )

  return (
    <dl className="grid grid-cols-2 divide-x divide-y divide-border border border-border bg-card lg:hidden">
      <Metric
        label="保有"
        value={holdingValue}
        detail={holdingsState === 'ready' ? '現在' : undefined}
      />
      <Metric
        label="次の決算"
        value={earningsValue}
        detail={
          earningsState === 'ready' && earnings != null
            ? (earnings.fiscal_period ?? earnings.title)
            : undefined
        }
      />
      <Metric
        label={per.label}
        value={valuationValue}
        detail={
          valuationState === 'ready' && valuation != null
            ? `${valuation.date} 時点`
            : undefined
        }
      />
      <Metric
        label="PBR"
        value={pbrValue}
        detail={
          valuationState === 'ready' && valuation != null
            ? `${valuation.date} 時点`
            : undefined
        }
      />
    </dl>
  )
}

function formatStateValue<T>(
  state: StockPageListState,
  data: T | null,
  empty: string,
  format: (value: T) => string,
): string {
  if (state === 'loading') return '読み込み中'
  if (state === 'error') return '取得失敗'
  if (data == null) return empty
  return format(data)
}

export function StockPageCompanyInfo({
  position,
  holdingsState,
  earnings,
  earningsState,
  valuation,
  valuationState,
}: {
  position: Position | null
  holdingsState: StockPageListState
  earnings: NextEarnings | null
  earningsState: StockPageListState
  valuation: Valuation | null
  valuationState: StockPageListState
}) {
  return (
    <aside className="space-y-4" aria-label="企業情報">
      <div className="hidden space-y-4 lg:block">
        <StockPageHoldings
          position={position}
          isLoading={holdingsState === 'loading'}
          hasError={holdingsState === 'error'}
        />
        <NextEarningsCard earnings={earnings} state={earningsState} />
        <ValuationCard valuation={valuation} state={valuationState} />
      </div>
      <MobileOverview
        position={position}
        holdingsState={holdingsState}
        earnings={earnings}
        earningsState={earningsState}
        valuation={valuation}
        valuationState={valuationState}
      />
    </aside>
  )
}
