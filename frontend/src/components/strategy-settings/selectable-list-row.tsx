import { Button } from '@fohte/ui/button'
import { ButtonGroup } from '@fohte/ui/button-group'
import { Chip } from '@fohte/ui/chip'
import type { ReactNode } from 'react'

interface SelectableListRowProps {
  children: ReactNode
  selected: boolean
  onSelect: () => void
  deleteLabel: string
  onDelete: () => void
  groupLabel?: string
  trailingControl?: ReactNode
}

export function SelectableListRow({
  children,
  selected,
  onSelect,
  deleteLabel,
  onDelete,
  groupLabel,
  trailingControl,
}: SelectableListRowProps) {
  return (
    <li className="border-b border-border px-1 py-1 font-mono text-xs last:border-b-0">
      <ButtonGroup className="w-full" aria-label={groupLabel}>
        <Button
          type="button"
          variant="ghost"
          onClick={onSelect}
          aria-pressed={selected}
          className="min-w-0 flex-1 justify-start"
        >
          {children}
          {selected && <Chip tone="strong">選択中</Chip>}
        </Button>
        {trailingControl}
        <Button
          type="button"
          variant="plain"
          onClick={onDelete}
          aria-label={deleteLabel}
          className="ml-1"
        >
          削除
        </Button>
      </ButtonGroup>
    </li>
  )
}
