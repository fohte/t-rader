import { Link } from '@tanstack/react-router'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import {
  formatNoteEvidencePercentage,
  formatNoteEvidencePrice,
  type NoteChangeEvidence,
} from '#lib/note-change-evidence'
import type { NotePriceReferenceField } from '#lib/note-price-reference'

const FIELD_LABELS: Record<NotePriceReferenceField, string> = {
  open: '始値',
  high: '高値',
  low: '安値',
  close: '終値',
  volume: '出来高',
}

type EvidenceDetails =
  | { kind: 'price'; date: string }
  | {
      kind: 'change'
      start: string
      end: string
      barsState:
        | { status: 'loading' }
        | { status: 'error' }
        | { status: 'empty' }
        | { status: 'ready'; evidence: NoteChangeEvidence }
    }

interface NoteEvidenceCardViewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  stockName: string | null
  stockId: string
  field: NotePriceReferenceField
  displayValue: string
  evidence: EvidenceDetails
}

export function NoteEvidenceCardView({
  open,
  onOpenChange,
  stockName,
  stockId,
  field,
  displayValue,
  evidence,
}: NoteEvidenceCardViewProps) {
  const stockLabel =
    stockName == null || stockName === '' ? stockId : `${stockName} ${stockId}`
  const fieldLabel = FIELD_LABELS[field]
  const title =
    evidence.kind === 'change'
      ? `${stockLabel} · ${fieldLabel}の騰落率`
      : `${stockLabel} · ${fieldLabel}`
  const linkDate = evidence.kind === 'change' ? evidence.end : evidence.date

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
        <p className="font-mono font-semibold text-foreground">{title}</p>
        {evidence.kind === 'price' ? (
          <div className="grid grid-cols-2 gap-x-3 gap-y-1">
            <span className="text-muted-foreground">時点</span>
            <span className="font-mono tabular-nums">{evidence.date}</span>
            <span className="text-muted-foreground">出どころ</span>
            <span>日足 (分割調整済み)</span>
          </div>
        ) : (
          <div className="space-y-1.5">
            <div className="grid grid-cols-2 gap-x-3 gap-y-1">
              <span className="text-muted-foreground">期間</span>
              <span className="font-mono tabular-nums">
                {evidence.start} → {evidence.end}
              </span>
              <span className="text-muted-foreground">出どころ</span>
              <span>日足 (分割調整済み)</span>
            </div>
            {evidence.barsState.status === 'loading' ? (
              <p role="status" className="text-muted-foreground">
                日足を読み込み中...
              </p>
            ) : evidence.barsState.status === 'error' ? (
              <p role="alert" className="text-primary">
                日足を取得できませんでした
              </p>
            ) : evidence.barsState.status === 'empty' ? (
              <p role="status" className="text-muted-foreground">
                計算に使える日足がありません
              </p>
            ) : (
              <div className="space-y-1">
                <p className="text-muted-foreground">計算</p>
                <p className="font-mono tabular-nums">
                  (
                  {formatNoteEvidencePrice(
                    evidence.barsState.evidence.endValue,
                  )}{' '}
                  −{' '}
                  {formatNoteEvidencePrice(
                    evidence.barsState.evidence.startValue,
                  )}
                  ){' ÷ '}
                  {formatNoteEvidencePrice(
                    evidence.barsState.evidence.startValue,
                  )}
                  {' = '}
                  {formatNoteEvidencePercentage(
                    evidence.barsState.evidence.changePercent,
                  )}
                  %
                </p>
                {!evidence.barsState.evidence.matchesSavedValue && (
                  <p className="text-primary">
                    書いた時点の値と今の日足が合わない可能性があります
                  </p>
                )}
              </div>
            )}
          </div>
        )}
        <Link
          to="/stocks/$stockId"
          params={{ stockId }}
          search={{ date: linkDate }}
          className="inline-flex items-center gap-1 border-t border-border pt-2 font-mono text-primary hover:underline"
        >
          銘柄ページで {linkDate} を見る <span aria-hidden="true">→</span>
        </Link>
      </PopoverContent>
    </Popover>
  )
}
