interface CreateHypothesisButtonProps {
  onClick: () => void
}

export function CreateHypothesisButton({
  onClick,
}: CreateHypothesisButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="h-9 border border-border bg-surface-strong px-3 font-mono text-xs text-muted-foreground-strong hover:border-primary hover:text-primary"
    >
      + 新規作成
    </button>
  )
}
