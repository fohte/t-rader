import { Button } from '@fohte/ui/button'
import {
  CommandPalette,
  type CommandPaletteGroup,
} from '@fohte/ui/command-palette'
import { ListItem } from '@fohte/ui/list'
import { Search } from 'lucide-react'
import type { MouseEventHandler, ReactNode } from 'react'

export function QuickSwitcherView({
  open,
  onOpenChange,
  shortcutLabel,
  value,
  onValueChange,
  groups,
  loading,
  emptyMessage,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  shortcutLabel: string
  value: string
  onValueChange: (value: string) => void
  groups: CommandPaletteGroup[]
  loading: boolean
  emptyMessage?: ReactNode
}) {
  return (
    <>
      <Button
        variant="secondary"
        size="sm"
        type="button"
        aria-label="探す"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={`探す ${shortcutLabel}`}
        onClick={() => {
          onOpenChange(!open)
        }}
      >
        <Search aria-hidden="true" className="size-3.5" />
        <span className="hidden md:inline">探す</span>
        <kbd className="hidden border border-border bg-surface-strong px-1 text-2xs text-muted-foreground md:inline">
          {shortcutLabel}
        </kbd>
      </Button>
      <CommandPalette
        open={open}
        onOpenChange={onOpenChange}
        value={value}
        onValueChange={onValueChange}
        groups={groups}
        loading={loading}
        emptyMessage={emptyMessage}
        inputLabel="画面・戦略・銘柄・指標を検索"
        dialogLabel="探す"
        placeholder="画面・戦略・銘柄・指標を検索"
        prefix={<Search className="size-4" aria-hidden="true" />}
      />
    </>
  )
}

export function QuickSwitcherItem({
  label,
  detail,
  isSelected,
  onMouseMove,
  onSelect,
}: {
  label: string
  detail?: string
  isSelected: boolean
  onMouseMove: MouseEventHandler<HTMLElement>
  onSelect: () => void
}) {
  return (
    <ListItem
      highlighted={isSelected}
      onMouseMove={onMouseMove}
      onSelect={onSelect}
    >
      <span className="min-w-0 truncate font-mono text-sm text-foreground">
        {label}
      </span>
      {detail != null && (
        <span className="ms-auto shrink-0 truncate text-xs text-muted-foreground">
          {detail}
        </span>
      )}
    </ListItem>
  )
}
