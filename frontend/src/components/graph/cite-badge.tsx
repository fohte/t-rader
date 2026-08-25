import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'
import { cn } from '#lib/utils'

interface CiteBadgeProps {
  number: number
  cite: string
  className?: string
}

export function CiteBadge({ number, cite, className }: CiteBadgeProps) {
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          onClick={(e) => {
            // 親ノードのクリック/ドラッグに伝播させない (ref-chip.tsx の onOpen と同じ理由)
            e.stopPropagation()
          }}
          className={cn(
            'border-border bg-surface-strong text-muted-foreground-strong hover:text-foreground hover:border-primary flex size-4 items-center justify-center rounded-full border font-mono text-[9px] leading-none',
            className,
          )}
        >
          {number}
        </button>
      </PopoverTrigger>
      <PopoverContent
        className="w-64 text-xs"
        onClick={(e) => {
          e.stopPropagation()
        }}
      >
        {cite}
      </PopoverContent>
    </Popover>
  )
}
