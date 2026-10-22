import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import {
  formatNoteEvidencePercentage,
  formatNoteEvidencePrice,
  type NoteEvidenceDetails,
} from '#lib/note-change-evidence'
import type { NotePriceReferenceField } from '#lib/note-price-reference'
import type { NoteValuationReferenceField } from '#lib/note-valuation-indicator-reference'

const PRICE_FIELD_LABELS: Record<NotePriceReferenceField, string> = {
  open: '始値',
  high: '高値',
  low: '安値',
  close: '終値',
  volume: '出来高',
}

const VALUATION_FIELD_LABELS: Record<NoteValuationReferenceField, string> = {
  per: 'PER',
  fwd_per: '予想 PER',
  pbr: 'PBR',
  roe: 'ROE',
  fwd_roe: '予想 ROE',
  mkt_cap: '時価総額',
  eps: 'EPS',
  fwd_eps: '予想 EPS',
  bps: 'BPS',
}

const VALUATION_SPLIT_ADJUSTED: Record<NoteValuationReferenceField, boolean> = {
  per: false,
  fwd_per: false,
  pbr: false,
  roe: false,
  fwd_roe: false,
  mkt_cap: false,
  eps: true,
  fwd_eps: true,
  bps: true,
}

interface NoteEvidenceValuation {
  kind: 'valuation'
  date: string
  field: NoteValuationReferenceField
}

interface NoteEvidenceIndicator {
  kind: 'indicator'
  date: string
}

type NoteEvidenceCardReference =
  | (NoteEvidenceDetails & {
      stockName: string | null
      stockId: string
      field: NotePriceReferenceField
    })
  | (NoteEvidenceValuation & {
      stockName: string | null
      stockId: string
    })
  | (NoteEvidenceIndicator & {
      indicatorName: string | null
      indicatorId: string
    })

interface NoteEvidenceCardViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  displayValue: string
  reference: NoteEvidenceCardReference
}

function stockLabel(stockName: string | null, stockId: string): string {
  return stockName == null || stockName === ''
    ? stockId
    : `${stockName} ${stockId}`
}

function referenceTitle(reference: NoteEvidenceCardReference): string {
  if (reference.kind === 'indicator') {
    return reference.indicatorName ?? reference.indicatorId
  }

  const label = stockLabel(reference.stockName, reference.stockId)
  if (reference.kind === 'valuation') {
    return `${label} · ${VALUATION_FIELD_LABELS[reference.field]}`
  }

  const fieldLabel = PRICE_FIELD_LABELS[reference.field]
  return reference.kind === 'change'
    ? `${label} · ${fieldLabel}の騰落率`
    : `${label} · ${fieldLabel}`
}

function evidenceSource(reference: NoteEvidenceCardReference): string {
  if (reference.kind === 'indicator') return 'マクロ指標'
  if (reference.kind === 'valuation') {
    return VALUATION_SPLIT_ADJUSTED[reference.field]
      ? 'バリュエーション (分割調整済み)'
      : 'バリュエーション'
  }
  return '日足 (分割調整済み)'
}

function EvidenceRows({ reference }: { reference: NoteEvidenceCardReference }) {
  return (
    <dl className="space-y-1">
      {reference.kind === 'change' ? (
        <>
          <div className="flex gap-x-3">
            <dt className="w-14 shrink-0 text-muted-foreground">期間</dt>
            <dd className="min-w-0 whitespace-nowrap font-mono tabular-nums">
              {reference.start} → {reference.end}
            </dd>
          </div>
        </>
      ) : (
        <div className="flex gap-x-3">
          <dt className="w-14 shrink-0 text-muted-foreground">時点</dt>
          <dd className="min-w-0 font-mono tabular-nums">{reference.date}</dd>
        </div>
      )}
      <div className="flex gap-x-3">
        <dt className="w-14 shrink-0 text-muted-foreground">出どころ</dt>
        <dd className="min-w-0">{evidenceSource(reference)}</dd>
      </div>
    </dl>
  )
}

function ChangeCalculation({
  reference,
}: {
  reference: Extract<NoteEvidenceDetails, { kind: 'change' }>
}) {
  const { barsState } = reference
  if (barsState.status === 'loading') {
    return (
      <p role="status" className="text-muted-foreground">
        日足を読み込み中...
      </p>
    )
  }
  if (barsState.status === 'error') {
    return (
      <p role="alert" className="text-destructive">
        日足を取得できませんでした
      </p>
    )
  }
  if (barsState.status === 'empty') {
    return (
      <p role="status" className="text-muted-foreground">
        計算に使える日足がありません
      </p>
    )
  }

  return (
    <div className="space-y-1">
      <p className="text-muted-foreground">計算</p>
      <p className="font-mono tabular-nums">
        ({formatNoteEvidencePrice(barsState.evidence.endValue)} −{' '}
        {formatNoteEvidencePrice(barsState.evidence.startValue)}) ÷{' '}
        {formatNoteEvidencePrice(barsState.evidence.startValue)} ={' '}
        {formatNoteEvidencePercentage(barsState.evidence.changePercent)}%
      </p>
      {!barsState.evidence.matchesSavedValue && (
        <p className="text-destructive">
          書いた時点の値と今の日足が合わない可能性があります
        </p>
      )}
    </div>
  )
}

function StockPageLink({
  reference,
}: {
  reference: Exclude<NoteEvidenceCardReference, { kind: 'indicator' }>
}) {
  const linkDate = reference.kind === 'change' ? reference.end : reference.date

  return (
    <Link
      to="/stocks/$stockId"
      params={{ stockId: reference.stockId }}
      search={{ date: linkDate }}
      className="inline-flex items-center gap-1 border-t border-border pt-2 font-mono text-foreground hover:underline"
    >
      銘柄ページで {linkDate} を見る <span aria-hidden="true">→</span>
    </Link>
  )
}

export function NoteEvidenceCardView({
  open,
  onOpenChange,
  displayValue,
  reference,
}: NoteEvidenceCardViewProps) {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger
        openOnHover
        render={
          <Button
            type="button"
            variant="plain"
            aria-label={`${displayValue} の根拠を開く`}
            title="根拠を見る"
            className="inline-flex items-baseline"
          >
            {displayValue}
          </Button>
        }
      />
      <PopoverContent align="start" padding="md" className="w-80">
        <div className="space-y-2 text-xs">
          <p className="font-mono font-semibold text-foreground">
            {referenceTitle(reference)}
          </p>
          <div className="space-y-1.5">
            <EvidenceRows reference={reference} />
            {reference.kind === 'change' && (
              <ChangeCalculation reference={reference} />
            )}
          </div>
          {reference.kind === 'indicator' ? (
            <Link
              to="/indicators/$indicatorId"
              params={{ indicatorId: reference.indicatorId }}
              className="inline-flex items-center gap-1 border-t border-border pt-2 font-mono text-foreground hover:underline"
            >
              指標ページを見る <span aria-hidden="true">→</span>
            </Link>
          ) : (
            <StockPageLink reference={reference} />
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
