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
      <button
        type="button"
        aria-label="探す"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={`探す ${shortcutLabel}`}
        onClick={() => {
          onOpenChange(!open)
        }}
        className="inline-flex h-7 w-7 flex-shrink-0 items-center justify-center gap-1.5 border border-border px-0 py-1 font-mono text-xs text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground md:h-auto md:w-auto md:px-2.5"
      >
        <Search aria-hidden="true" className="size-3.5" />
        <span className="hidden md:inline">探す</span>
        <kbd className="hidden border border-border bg-surface-strong px-1 text-2xs text-muted-foreground md:inline">
          {shortcutLabel}
        </kbd>
      </button>
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
        footer={
          <div className="flex min-h-9 flex-wrap items-center gap-1.5 border-t border-border px-4 py-2 font-mono text-2xs text-muted-foreground-ghost">
            <KeyHint>↑↓</KeyHint>
            <span>移動</span>
            <KeyHint>Enter</KeyHint>
            <span>開く</span>
            <KeyHint>Esc</KeyHint>
            <span>閉じる</span>
          </div>
        }
      />
    </>
  )
}

function KeyHint({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex min-w-11 items-center justify-center rounded-(--keycap-radius) border border-border bg-secondary px-1.5 py-0.5 text-center text-foreground">
      {children}
    </span>
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
