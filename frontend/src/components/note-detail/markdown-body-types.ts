import type { ComponentType } from 'react'
import type { ExtraProps } from 'react-markdown'

import type { MarkdownBodyRangeCommentProps } from '#components/note-detail/markdown-body-range-comment-context'
import type { ResolvedRef } from '#hooks/use-resolve-ref'
import type { components } from '#lib/api/schema.gen'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'
import type { ParsedNotePriceReference } from '#lib/note-price-reference'
import type {
  ParsedNoteIndicatorObservationReference,
  ParsedNoteValuationReference,
} from '#lib/note-valuation-indicator-reference'

type ApiGraphDef = components['schemas']['GraphDef']
type NoteLinkItem = components['schemas']['NoteLinkItem']
export type MarkdownNode = NonNullable<ExtraProps['node']>

export interface MarkdownBodyProps {
  source: string
  graphs?: ApiGraphDef[]
  noteLinks?: NoteLinkItem[]
  resolvedPriceReferences?: components['schemas']['Value']
  onAnno?: (id: string) => void
  onRef?: (token: string, resolved: ResolvedRef) => void
  rangeComments?: MarkdownBodyRangeCommentProps
}

// remarkNoteTokens が data.hName で生成するタグ名に対応する。
export type NoteTokenComponents = {
  'note-ref': ComponentType<{
    token: string
    commentTextHighlight?: string
  }>
  'note-anno': ComponentType<{ annoId: string }>
  'note-graph': ComponentType<{ graphId: string; node?: MarkdownNode }>
  'note-link': ComponentType<{ noteId: string; token: string }>
  'note-price-reference': ComponentType<
    ParsedNotePriceReference & { value: string }
  >
  'note-eval-reference': ComponentType<{
    evaluationId: string
    outputName: string
    value: string
  }>
  'note-valuation-reference': ComponentType<
    ParsedNoteValuationReference & { value: string }
  >
  'note-indicator-reference': ComponentType<
    ParsedNoteIndicatorObservationReference & { value: string }
  >
  'note-change-reference': ComponentType<
    ParsedNoteChangeReference & { token: string; value?: string }
  >
  'note-change-figure': ComponentType<
    ParsedNoteChangeReference & { node?: MarkdownNode }
  >
}
