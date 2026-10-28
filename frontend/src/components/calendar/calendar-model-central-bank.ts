import type { components } from '#lib/api/schema.gen'

type CalendarEvent = components['schemas']['CalendarEventResponse']
type Event = Extract<CalendarEvent, { kind: 'event' }>
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

export type CalendarCentralBankStatus = {
  phase?: 'first_day' | 'decision_day'
  label: string
  references: CalendarEventReferenceView[]
}

export function buildCentralBankStatus(
  event: Event,
  today: string,
): CalendarCentralBankStatus {
  const individualDocuments = event.references.filter(
    (reference) =>
      reference.kind === 'individual_document' && reference.url != null,
  )
  const listings = event.references.filter(
    (reference) =>
      reference.kind === 'official_listing' && reference.url != null,
  )
  const phase = getCentralBankMeetingPhase(event)
  const isUnconfirmedBojMeeting =
    event.source === 'boj' &&
    event.external_id.startsWith('meeting-') &&
    phase == null
  const eligibleIndividualDocuments = isUnconfirmedBojMeeting
    ? []
    : individualDocuments
  const scheduleReferences = listings.filter(
    (reference) => reference.target_period_type === 'official_schedule',
  )
  const scheduled = event.event_date >= today && scheduleReferences.length > 0

  const label =
    phase === 'first_day'
      ? scheduled
        ? '発表予定'
        : listings.length > 0
          ? '公式一覧のみ'
          : '遷移先未確認 / 非リンク'
      : eligibleIndividualDocuments.length > 0
        ? '個別資料あり'
        : scheduled
          ? '発表予定'
          : listings.length > 0
            ? '公式一覧のみ'
            : '遷移先未確認 / 非リンク'
  const selectedReferences =
    phase === 'first_day'
      ? scheduled
        ? scheduleReferences
        : listings
      : eligibleIndividualDocuments.length > 0
        ? eligibleIndividualDocuments
        : scheduled
          ? scheduleReferences
          : listings

  return {
    ...(phase == null ? {} : { phase }),
    label,
    references: selectedReferences.map(toCentralBankReferenceView),
  }
}

function getCentralBankMeetingPhase(
  event: Event,
): 'first_day' | 'decision_day' | undefined {
  if (event.source === 'boj' && event.external_id.startsWith('meeting-')) {
    const meetingDayMarker = /(?:\((\d+)日目\)|（(\d+)日目）)$/u.exec(
      event.title,
    )
    const meetingDayText = meetingDayMarker?.[1] ?? meetingDayMarker?.[2]
    if (meetingDayText == null) return undefined

    const meetingDay = Number(meetingDayText)
    if (!Number.isSafeInteger(meetingDay) || meetingDay < 1) return undefined

    return meetingDay === 1 ? 'first_day' : 'decision_day'
  }
  if (event.source === 'fed') return 'decision_day'
  if (event.source === 'ecb') {
    return event.event_at == null ? 'first_day' : 'decision_day'
  }
  return undefined
}

function toCentralBankReferenceView(
  reference: EventReference,
): CalendarEventReferenceView {
  const view = toCalendarEventReferenceView(reference)
  const referenceTypeLabel = formatCentralBankReferenceType(
    reference.target_period_type,
  )
  return {
    ...view,
    ...(referenceTypeLabel == null ? {} : { referenceTypeLabel }),
  }
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

const CENTRAL_BANK_REFERENCE_TYPE_LABELS: Readonly<Record<string, string>> = {
  official_schedule: '公式日程',
  official_archive: '公式一覧',
  meeting: '会合資料',
  tankan: '短観',
}

function formatCentralBankReferenceType(
  value: string | null | undefined,
): string | undefined {
  return value == null ? undefined : CENTRAL_BANK_REFERENCE_TYPE_LABELS[value]
}
