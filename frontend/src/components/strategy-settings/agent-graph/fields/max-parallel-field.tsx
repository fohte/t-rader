import { Input } from '#components/ui/input'

interface MaxParallelFieldProps {
  value: number | undefined
  onChange: (next: number | undefined) => void
}

export function MaxParallelField({ value, onChange }: MaxParallelFieldProps) {
  return (
    <>
      <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
        並列上限
      </span>
      <Input
        aria-label="並列上限"
        type="number"
        min={1}
        value={value ?? ''}
        onChange={(e) => {
          const next =
            e.target.value === '' ? undefined : Number(e.target.value)
          onChange(next == null || Number.isNaN(next) ? undefined : next)
        }}
        className="h-auto w-20 rounded-none border-border bg-background py-1 font-mono text-[11.5px] text-foreground"
      />
    </>
  )
}
