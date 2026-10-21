import { Button } from '@fohte/ui/button'
import { ChevronDown } from 'lucide-react'
import type { ReactNode } from 'react'

export function WidgetCollapsibleSection({
  label,
  expanded,
  onExpandedChange,
  children,
}: {
  label: string
  expanded: boolean
  onExpandedChange: (expanded: boolean) => void
  children?: ReactNode
}) {
  return (
    <div className="border border-border bg-card">
      <Button
        aria-expanded={expanded}
        className="min-h-10 w-full justify-start whitespace-normal text-left"
        onClick={() => {
          onExpandedChange(!expanded)
        }}
        type="button"
        variant="ghost"
      >
        {label}
        <ChevronDown
          aria-hidden="true"
          className="ml-auto size-4 transition-transform data-[expanded=true]:rotate-180"
          data-expanded={String(expanded)}
        />
      </Button>
      {expanded && <div className="border-t border-border">{children}</div>}
    </div>
  )
}
