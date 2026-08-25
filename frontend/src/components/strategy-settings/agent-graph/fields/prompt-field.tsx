interface PromptFieldProps {
  value: string
  onChange: (next: string) => void
}

export function PromptField({ value, onChange }: PromptFieldProps) {
  return (
    <>
      <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
        プロンプト
      </span>
      <textarea
        aria-label="プロンプト"
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
        }}
        rows={3}
        className="w-full resize-y border border-border bg-background p-2 font-mono text-xs leading-relaxed text-muted-foreground-strong outline-none focus:border-muted-foreground"
      />
    </>
  )
}
