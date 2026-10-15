import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'

import { useChangeReferenceBars } from '#components/note-detail/change-reference-figure'
import {
  NoteEvaluationEvidenceCardView,
  type NoteEvaluationEvidenceState,
} from '#components/note-detail/note-evaluation-evidence-card-view'
import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import { useResolveRef } from '#hooks/use-resolve-ref'
import { fetchClient } from '#lib/api/client'
import {
  formatNoteEvidencePercentage,
  formatNoteEvidencePrice,
  type NoteEvidenceDetails,
  resolveNoteChangeEvidence,
} from '#lib/note-change-evidence'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'
import type { ParsedNotePriceReference } from '#lib/note-price-reference'
import type {
  ParsedNoteIndicatorObservationReference,
  ParsedNoteValuationReference,
} from '#lib/note-valuation-indicator-reference'

type StockEvidence =
  | (NoteEvidenceDetails & { field: ParsedNotePriceReference['field'] })
  | {
      kind: 'valuation'
      date: string
      field: ParsedNoteValuationReference['field']
    }

interface ResolvedStockEvidenceReferenceProps {
  instrumentId: string
  displayValue: string
  evidence: StockEvidence
}

function ResolvedStockEvidenceReference({
  instrumentId,
  displayValue,
  evidence,
}: ResolvedStockEvidenceReferenceProps) {
  const [open, setOpen] = useState(false)
  const stock = useResolveRef(`stock:${instrumentId}`)

  return (
    <NoteEvidenceCardView
      open={open}
      onOpenChange={setOpen}
      displayValue={displayValue}
      reference={{
        ...evidence,
        stockName: stock.name,
        stockId: stock.id,
      }}
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
    <ResolvedStockEvidenceReference
      instrumentId={instrumentId}
      displayValue={formatNoteEvidencePrice(numericValue)}
      evidence={{ kind: 'price', date, field }}
    />
  )
}

export function NoteValuationReference({
  instrumentId,
  date,
  field,
  value,
}: ParsedNoteValuationReference & { value: string }) {
  return (
    <ResolvedStockEvidenceReference
      instrumentId={instrumentId}
      displayValue={formatNoteEvidencePrice(Number(value))}
      evidence={{
        kind: 'valuation',
        date,
        field,
      }}
    />
  )
}

export function NoteIndicatorObservationReference({
  indicatorId,
  date,
  value,
}: ParsedNoteIndicatorObservationReference & { value: string }) {
  const [open, setOpen] = useState(false)
  const indicator = useResolveRef(`indicator:${indicatorId}`)

  return (
    <NoteEvidenceCardView
      open={open}
      onOpenChange={setOpen}
      displayValue={formatNoteEvidencePrice(Number(value))}
      reference={{
        kind: 'indicator',
        date,
        indicatorName: indicator.name,
        indicatorId: indicator.id,
      }}
    />
  )
}

export function NoteEvaluationEvidenceReference({
  evaluationId,
  outputName,
  value,
}: {
  evaluationId: string
  outputName: string
  value: string
}) {
  const [open, setOpen] = useState(false)
  const evaluationQuery = useQuery({
    queryKey: ['custom-indicator-evaluation', evaluationId],
    queryFn: async ({ signal }) => {
      const result = await fetchClient.GET(
        '/api/custom-indicator-evaluations/{evaluation_id}',
        {
          params: { path: { evaluation_id: evaluationId } },
          signal,
        },
      )
      if (result.response.status === 404)
        return { status: 'not-found' as const }
      if (!result.response.ok || result.data == null) {
        return { status: 'error' as const }
      }
      return { status: 'ready' as const, evaluation: result.data }
    },
    enabled: open,
    retry: false,
  })

  const state: NoteEvaluationEvidenceState = evaluationQuery.isPending
    ? { status: 'loading' as const }
    : evaluationQuery.isError
      ? { status: 'error' as const }
      : evaluationQuery.data

  return (
    <NoteEvaluationEvidenceCardView
      open={open}
      onOpenChange={setOpen}
      displayValue={formatNoteEvidencePrice(Number(value))}
      outputName={outputName}
      state={state}
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
    <ResolvedStockEvidenceReference
      instrumentId={instrumentId}
      displayValue={`${formatNoteEvidencePercentage(numericValue, 0)}%`}
      evidence={{ kind: 'change', start, end, barsState, field }}
    />
  )
}
