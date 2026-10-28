import type { components } from '#lib/api/schema.gen'

type EventReference = components['schemas']['CalendarEventReferenceResponse']

export type CalendarEventReferenceView = {
  key: string
  title: string
  source: string
  checkedAt: string
  targetPeriodStart?: string
  targetPeriodEnd?: string
  targetPeriodType?: string
  href?: string
  publicationEvidence?: string
  publicationEvidenceHref?: string
  referenceTypeLabel?: string
}

export function toCalendarEventReferenceView(
  reference: EventReference,
): CalendarEventReferenceView {
  const targetPeriodType =
    reference.target_period_type == null
      ? undefined
      : formatIndicatorPeriodType(reference.target_period_type)
  return {
    key: `${reference.kind}:${reference.source}:${reference.external_id}`,
    title: reference.title,
    source: reference.source,
    checkedAt: reference.checked_at,
    ...(reference.target_period_start == null
      ? {}
      : { targetPeriodStart: reference.target_period_start }),
    ...(reference.target_period_end == null
      ? {}
      : { targetPeriodEnd: reference.target_period_end }),
    ...(targetPeriodType == null ? {} : { targetPeriodType }),
    ...(reference.url == null ? {} : { href: reference.url }),
    ...(reference.publication_evidence == null
      ? {}
      : { publicationEvidence: reference.publication_evidence }),
    ...(reference.publication_evidence_url == null
      ? {}
      : { publicationEvidenceHref: reference.publication_evidence_url }),
  }
}

const INDICATOR_PERIOD_TYPE_LABELS: Readonly<Record<string, string>> = {
  month: '月次',
  quarter: '四半期',
  half_year: '半期',
  fiscal_year: '年度',
  month_range: '複数月',
  year: '年次',
}

function formatIndicatorPeriodType(value: string): string | undefined {
  return INDICATOR_PERIOD_TYPE_LABELS[value]
}
