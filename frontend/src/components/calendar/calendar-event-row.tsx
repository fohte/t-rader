import { Link } from '@tanstack/react-router'

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
  const titleTone = emphasized
    ? 'text-destructive'
    : target
      ? 'font-semibold text-foreground'
      : muted
        ? 'text-muted-foreground'
        : 'text-foreground'
  const titleClassName = `min-w-0 flex-1 break-words ${titleTone}`

  return (
    <div className="flex items-start gap-1.5 px-3 py-1 sm:gap-2 sm:px-4">
      <span className="w-18 shrink-0 pt-0.5 text-right tabular-nums text-muted-foreground">
        {time}
      </span>
      <span className="w-10 shrink-0 border border-border px-1 py-0.5 text-center text-2xs text-muted-foreground-strong">
        {country}
      </span>
      <span className="w-12 shrink-0 border border-border px-1 py-0.5 text-center text-2xs text-muted-foreground-strong">
        {category}
      </span>
      {titleAction != null ? (
        <button
          type="button"
          aria-expanded={titleAction.expanded}
          aria-controls={titleAction.controlsId}
          onClick={titleAction.onClick}
          className={`${titleClassName} text-left hover:text-foreground hover:underline`}
        >
          {title}
        </button>
      ) : stockId != null ? (
        <Link
          to="/charts/$instrumentId"
          params={{ instrumentId: stockId }}
          className={`${titleClassName} hover:underline`}
        >
          {title}
        </Link>
      ) : (
        <span className={titleClassName}>{title}</span>
      )}
    </div>
  )
}
