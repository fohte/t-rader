import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'
import clsx from 'clsx'

import type {
  CalendarEarningsReferenceView,
  CalendarEarningsStatus,
} from '#components/calendar/calendar-model'

const checkedAtFormatter = new Intl.DateTimeFormat('ja-JP', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  timeZone: 'Asia/Tokyo',
})

function formatCheckedAt(value: string): string {
  return checkedAtFormatter.format(new Date(value))
}

function EarningsReference({
  reference,
}: {
  reference: CalendarEarningsReferenceView
}) {
  return (
    <div className="flex min-w-0 flex-col border-t border-border/60 py-1 first:border-t-0">
      {reference.href == null ? (
        <span className="inline-flex min-h-11 min-w-0 items-center break-words text-muted-foreground">
          {reference.title} (遷移先未確認)
        </span>
      ) : (
        <a
          href={reference.href}
          target="_blank"
          rel="noreferrer"
          className="inline-flex min-h-11 min-w-0 items-center break-words text-foreground underline decoration-border underline-offset-4 hover:decoration-foreground"
        >
          {reference.title} →
        </a>
      )}
      {(reference.targetPeriodEnd != null ||
        reference.targetPeriodType != null) && (
        <span className="text-2xs text-muted-foreground">
          対象期: {reference.targetPeriodEnd ?? '期末未確認'}{' '}
          {reference.targetPeriodType ?? ''}
        </span>
      )}
      <span className="text-2xs text-muted-foreground">
        取得元: {reference.source} / 確認日時:{' '}
        {formatCheckedAt(reference.checkedAt)}
      </span>
      {reference.publicationEvidence != null && (
        <div className="flex min-h-11 min-w-0 items-center gap-1 text-2xs text-muted-foreground">
          <span>公表根拠:</span>
          {reference.publicationEvidenceHref == null ? (
            <span className="break-words">{reference.publicationEvidence}</span>
          ) : (
            <a
              href={reference.publicationEvidenceHref}
              target="_blank"
              rel="noreferrer"
              className="inline-flex min-h-11 items-center break-words underline decoration-border underline-offset-4 hover:decoration-foreground"
            >
              {reference.publicationEvidence}
            </a>
          )}
        </div>
      )}
    </div>
  )
}

function EarningsStatus({ status }: { status: CalendarEarningsStatus }) {
  return (
    <div className="mt-1 min-w-0 border-l-2 border-border pl-2 text-2xs">
      <p className="min-h-6 text-muted-foreground">{status.label}</p>
      {status.internalTarget != null && (
        <Link
          to="/stocks/$stockId"
          params={{ stockId: status.internalTarget.stockId }}
          search={{
            stockTab: 'financials',
            periodEnd: status.internalTarget.periodEnd,
            periodType: status.internalTarget.periodType,
          }}
          className="inline-flex min-h-11 items-center text-foreground underline decoration-border underline-offset-4 hover:decoration-foreground"
        >
          該当期の実績を見る →
        </Link>
      )}
      {status.references.map((reference) => (
        <EarningsReference key={reference.key} reference={reference} />
      ))}
    </div>
  )
}

export function CalendarEventRow({
  time,
  country,
  category,
  title,
  stockId,
  earningsStatus,
  emphasized,
  target,
  muted,
  titleAction,
}: {
  time: string
  country: string
  category: string
  title: string
  stockId?: string
  earningsStatus?: CalendarEarningsStatus
  emphasized: boolean
  target: boolean
  muted: boolean
  titleAction?: {
    expanded: boolean
    controlsId?: string
    onClick: () => void
  }
}) {
  const titleToneClassName = clsx(
    emphasized && 'text-destructive',
    !emphasized && target && 'font-semibold text-foreground',
    !emphasized && !target && muted && 'text-muted-foreground',
    !emphasized && !target && !muted && 'text-foreground',
  )

  return (
    <div className="flex items-start gap-1.5 px-3 py-1 sm:gap-2 sm:px-4">
      <span className="w-18 shrink-0 pt-0.5 text-right tabular-nums text-muted-foreground">
        {time}
      </span>
      <div className="flex w-10 shrink-0 justify-center">
        <Chip>{country}</Chip>
      </div>
      <div className="flex w-12 shrink-0 justify-center">
        <Chip>{category}</Chip>
      </div>
      <div className="min-w-0 flex-1 py-0.5">
        {titleAction != null ? (
          <Button
            type="button"
            variant="plain"
            aria-expanded={titleAction.expanded}
            aria-controls={titleAction.controlsId}
            onClick={titleAction.onClick}
            className="min-h-11 w-full min-w-0 justify-start text-left"
          >
            <span
              className={clsx(
                titleToneClassName,
                'w-full break-words text-left',
              )}
            >
              {title}
            </span>
          </Button>
        ) : stockId != null ? (
          <Link
            to="/stocks/$stockId"
            params={{ stockId }}
            className={clsx(
              'flex min-h-11 min-w-0 items-center',
              titleToneClassName,
              'hover:underline',
            )}
          >
            <span className="w-full break-words">{title}</span>
          </Link>
        ) : (
          <span
            className={clsx('min-h-11 min-w-0 break-words', titleToneClassName)}
          >
            {title}
          </span>
        )}
        {earningsStatus != null && <EarningsStatus status={earningsStatus} />}
      </div>
    </div>
  )
}
