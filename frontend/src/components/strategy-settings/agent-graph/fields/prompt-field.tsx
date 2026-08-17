interface PromptFieldProps {
  value: string
  onChange: (next: string) => void
}

export function PromptField({ value, onChange }: PromptFieldProps) {
  return (
    <>
      <span className="pt-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        プロンプト
      </span>
      <textarea
        aria-label="プロンプト"
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
        }}
        rows={3}
        className="w-full resize-y border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] p-2 font-mono text-[11.5px] leading-relaxed text-[color:var(--color-text-secondary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
      />
    </>
  )
}
