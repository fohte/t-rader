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

function EvidenceRows({ reference }: { reference: NoteEvidenceCardReference }) {
  return (
    <div
      className="grid gap-x-3 gap-y-1"
      style={{ gridTemplateColumns: '3.5rem minmax(0, 1fr)' }}
    >
      {reference.kind === 'change' ? (
        <>
          <span className="text-muted-foreground">期間</span>
          <span className="whitespace-nowrap font-mono tabular-nums">
            {reference.start} → {reference.end}
          </span>
        </>
      ) : (
        <>
          <span className="text-muted-foreground">時点</span>
          <span className="font-mono tabular-nums">{reference.date}</span>
        </>
      )}
      {(reference.kind === 'valuation' || reference.kind === 'indicator') && (
        <>
          <span className="text-muted-foreground">計算</span>
          <span>
            {reference.kind === 'valuation' &&
            VALUATION_SPLIT_ADJUSTED[reference.field]
              ? 'なし (分割調整済み)'
              : 'なし'}
          </span>
        </>
      )}
      <span className="text-muted-foreground">出どころ</span>
      <span>
        {reference.kind === 'price' || reference.kind === 'change'
          ? '日足 (分割調整済み)'
          : reference.kind === 'valuation'
            ? 'バリュエーション'
            : 'マクロ指標'}
      </span>
    </div>
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
      <p role="alert" className="text-primary">
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
        <p className="text-primary">
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
      className="inline-flex items-center gap-1 border-t border-border pt-2 font-mono text-primary hover:underline"
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
          <button
            type="button"
            aria-label={`${displayValue} の根拠を開く`}
            title="根拠を見る"
            className="inline-flex cursor-pointer items-baseline border-b border-dotted border-primary/60 bg-transparent p-0 font-mono font-medium tabular-nums text-foreground hover:border-primary hover:text-primary"
          >
            {displayValue}
          </button>
        }
      />
      <PopoverContent
        align="start"
        className="w-80 space-y-2 p-3 text-xs"
        style={{ maxWidth: 'calc(100vw - 1.5rem)' }}
      >
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
            className="inline-flex items-center gap-1 border-t border-border pt-2 font-mono text-primary hover:underline"
          >
            指標ページを見る <span aria-hidden="true">→</span>
          </Link>
        ) : (
          <StockPageLink reference={reference} />
        )}
      </PopoverContent>
    </Popover>
  )
}
