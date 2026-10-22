import { SegmentedControl } from '@fohte/ui/segmented-control'

export interface FilterOption {
  id: string
  label: string
  count: number
}

export function FilterBar({
  options,
  value,
  onChange,
  allLabel,
  allCount,
}: {
  options: FilterOption[]
  value: string
  onChange: (v: string) => void
  allLabel: string
  allCount: number
}) {
  const segmentedOptions = [
    { value: 'all', label: `${allLabel} (${String(allCount)})` },
    ...options.map((option) => ({
      value: option.id,
      label: `${option.label} (${String(option.count)})`,
    })),
  ]

  return (
    <div className="font-mono">
      <SegmentedControl
        aria-label="絞り込み"
        options={segmentedOptions}
        value={value}
        onValueChange={onChange}
      />
    </div>
  )
}
