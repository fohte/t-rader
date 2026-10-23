import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'
import clsx from 'clsx'

export function CalendarEventRow({
  time,
  country,
  category,
  title,
  stockId,
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
      {titleAction != null ? (
        <Button
          type="button"
          variant="plain"
          aria-expanded={titleAction.expanded}
          aria-controls={titleAction.controlsId}
          onClick={titleAction.onClick}
          className="min-h-11 min-w-0 flex-1 justify-start text-left"
        >
          <span
            className={clsx(titleToneClassName, 'w-full break-words text-left')}
          >
            {title}
          </span>
        </Button>
      ) : stockId != null ? (
        <Link
          to="/stocks/$stockId"
          params={{ stockId }}
          className={clsx(
            'flex min-h-11 min-w-0 flex-1 items-center',
            titleToneClassName,
            'hover:underline',
          )}
        >
          <span className="w-full break-words">{title}</span>
        </Link>
      ) : (
        <span
          className={clsx('min-w-0 flex-1 break-words', titleToneClassName)}
        >
          {title}
        </span>
      )}
    </div>
  )
}
