import { Chip } from '@fohte/ui/chip'
import { Input } from '@fohte/ui/input'

interface MaxParallelFieldProps {
  value: number | undefined
  onChange: (next: number | undefined) => void
}

export function MaxParallelField({ value, onChange }: MaxParallelFieldProps) {
  return (
    <>
      <Chip>並列上限</Chip>
      <div className="w-20 font-mono">
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
        />
      </div>
    </>
  )
}
