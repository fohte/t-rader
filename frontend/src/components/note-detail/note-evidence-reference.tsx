import { useState } from 'react'

import { useChangeReferenceBars } from '#components/note-detail/change-reference-figure'
import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import { useResolveRef } from '#hooks/use-resolve-ref'
import {
  formatNoteEvidencePercentage,
  formatNoteEvidencePrice,
  type NoteEvidenceDetails,
  resolveNoteChangeEvidence,
} from '#lib/note-change-evidence'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'
import type { ParsedNotePriceReference } from '#lib/note-price-reference'

interface ResolvedNoteEvidenceReferenceProps {
  instrumentId: string
  field: ParsedNotePriceReference['field']
  displayValue: string
  evidence: NoteEvidenceDetails
}

function ResolvedNoteEvidenceReference({
  instrumentId,
  field,
  displayValue,
  evidence,
}: ResolvedNoteEvidenceReferenceProps) {
  const [open, setOpen] = useState(false)
  const stock = useResolveRef(`stock:${instrumentId}`)

  return (
    <NoteEvidenceCardView
      open={open}
      onOpenChange={setOpen}
      stockName={stock.name}
      stockId={stock.id}
      field={field}
      displayValue={displayValue}
      evidence={evidence}
    />
  )
}

export function NotePriceReference({
  instrumentId,
  date,
  field,
  value,
}: ParsedNotePriceReference & { value: string }) {
  const numericValue = Number(value)
  return (
    <ResolvedNoteEvidenceReference
      instrumentId={instrumentId}
      field={field}
      displayValue={formatNoteEvidencePrice(numericValue)}
      evidence={{ kind: 'price', date }}
    />
  )
}

export function NoteChangeReference({
  instrumentId,
  start,
  end,
  field,
  token,
  value,
}: ParsedNoteChangeReference & { token: string; value?: string }) {
  if (value == null) {
    return <span className="font-mono font-medium tabular-nums">{token}</span>
  }

  return (
    <ResolvedNoteChangeReference
      instrumentId={instrumentId}
      start={start}
      end={end}
      field={field}
      value={value}
    />
  )
}

function ResolvedNoteChangeReference({
  instrumentId,
  start,
  end,
  field,
  value,
}: ParsedNoteChangeReference & { value: string }) {
  const reference: ParsedNoteChangeReference = {
    instrumentId,
    start,
    end,
    field,
  }
  const result = useChangeReferenceBars(reference)
  const numericValue = Number(value)
  const calculatedEvidence = resolveNoteChangeEvidence(
    result.data ?? [],
    reference,
    numericValue,
  )
  const barsState = result.isPending
    ? { status: 'loading' as const }
    : result.isError
      ? { status: 'error' as const }
      : calculatedEvidence == null
        ? { status: 'empty' as const }
        : { status: 'ready' as const, evidence: calculatedEvidence }

  return (
    <ResolvedNoteEvidenceReference
      instrumentId={instrumentId}
      field={field}
      displayValue={`${formatNoteEvidencePercentage(numericValue, 0)}%`}
      evidence={{ kind: 'change', start, end, barsState }}
    />
  )
}
