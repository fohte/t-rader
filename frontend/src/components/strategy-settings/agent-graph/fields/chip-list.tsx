import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'
import { List, ListItem } from '@fohte/ui/list'
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
  /** Storybook で候補 popup を開いた状態を描画するための初期値 */
  defaultOpen?: boolean
  extra?: ReactNode
}

export function ChipList({
  values,
  options,
  onAdd,
  onRemove,
  removeAriaLabel,
  addAriaLabel,
  defaultOpen = false,
  extra,
}: ChipListProps) {
  const [open, setOpen] = useState(defaultOpen)
  const remaining = options.filter((o) => !values.includes(o))

  return (
    <div className="flex flex-wrap items-center gap-1 font-mono text-2xs">
      {values.map((name) => (
        <Chip
          key={name}
          size="md"
          tone="strong"
          onRemove={() => {
            onRemove(name)
          }}
          removeLabel={removeAriaLabel(name)}
        >
          {name}
        </Chip>
      ))}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger
          render={
            <Button type="button" variant="plain" aria-label={addAriaLabel}>
              + 追加
            </Button>
          }
        />
        <PopoverContent className="w-48" padding="none" align="start">
          {remaining.length === 0 ? (
            <p className="px-2 py-1 font-mono text-2xs text-muted-foreground">
              追加できる候補がありません
            </p>
          ) : (
            <List>
              {remaining.map((name) => (
                <ListItem
                  key={name}
                  onSelect={() => {
                    onAdd(name)
                    setOpen(false)
                  }}
                >
                  <span className="truncate font-mono text-xs">{name}</span>
                </ListItem>
              ))}
            </List>
          )}
        </PopoverContent>
      </Popover>
      {extra}
    </div>
  )
}
