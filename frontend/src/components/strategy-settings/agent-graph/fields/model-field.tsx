import { Input } from '#components/ui/input'

interface ModelFieldProps {
  value: string
  onChange: (next: string) => void
}

export function ModelField({ value, onChange }: ModelFieldProps) {
  return (
    <>
      <span className="pt-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        モデル
      </span>
      <Input
        aria-label="モデル"
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
        }}
        className="h-auto w-full max-w-sm rounded-none border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] py-1 font-mono text-[11.5px] text-[color:var(--color-text-primary)]"
      />
    </>
  )
}
