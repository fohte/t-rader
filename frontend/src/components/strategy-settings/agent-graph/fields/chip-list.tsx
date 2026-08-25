import type { ReactNode } from 'react'
import { useState } from 'react'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

interface ChipListProps {
  values: string[]
  options: string[]
  onAdd: (name: string) => void
  onRemove: (name: string) => void
  removeAriaLabel: (name: string) => string
  /** 同じカード内に複数の ChipList が並ぶため、「+ 追加」ボタンを区別するラベル */
  addAriaLabel: string
  extra?: ReactNode
}

export function ChipList({
  values,
  options,
  onAdd,
  onRemove,
  removeAriaLabel,
  addAriaLabel,
  extra,
}: ChipListProps) {
  const [open, setOpen] = useState(false)
  const remaining = options.filter((o) => !values.includes(o))

  return (
    <div className="flex flex-wrap items-center gap-1">
      {values.map((name) => (
        <span
          key={name}
          className="inline-flex items-center gap-1 border border-border bg-bg-tertiary px-1.5 py-0.5 font-mono text-2xs text-muted-foreground-strong"
        >
          {name}
          <button
            type="button"
            aria-label={removeAriaLabel(name)}
            onClick={() => {
              onRemove(name)
            }}
            className="text-muted-foreground hover:text-primary"
          >
            ×
          </button>
        </span>
      ))}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger
          render={
            <button
              type="button"
              aria-label={addAriaLabel}
              className="border border-dashed border-border px-1.5 py-0.5 font-mono text-2xs text-muted-foreground hover:text-foreground"
            >
              + 追加
            </button>
          }
        />
        <PopoverContent className="w-48 p-1" align="start">
          {remaining.length === 0 ? (
            <p className="px-2 py-1 font-mono text-2xs text-muted-foreground">
              追加できる候補がありません
            </p>
          ) : (
            <ul>
              {remaining.map((name) => (
                <li key={name}>
                  <button
                    type="button"
                    onClick={() => {
                      onAdd(name)
                      setOpen(false)
                    }}
                    className="w-full truncate px-2 py-1 text-left font-mono text-xs hover:bg-surface-strong"
                  >
                    {name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </PopoverContent>
      </Popover>
      {extra}
    </div>
  )
}
