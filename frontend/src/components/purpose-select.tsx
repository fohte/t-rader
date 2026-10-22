import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

export interface PurposeSelectProps {
  purposes: string[]
  selectedPurpose: string
  onPurposeChange: (value: string) => void
  disabled: boolean
}

export function PurposeSelect({
  purposes,
  selectedPurpose,
  onPurposeChange,
  disabled,
}: PurposeSelectProps): React.ReactElement {
  const defaultPurposeLabel = '既定の設定'
  const selectablePurposes =
    selectedPurpose === '' || purposes.includes(selectedPurpose)
      ? purposes
      : [selectedPurpose, ...purposes]
  const items = [
    { value: null, label: defaultPurposeLabel },
    ...selectablePurposes.map((purpose) => ({
      value: purpose,
      label: purpose,
    })),
  ]

  return (
    <label className="flex items-center gap-2 font-mono text-xs">
      <span className="text-muted-foreground">実行目的</span>
      <Select
        items={items}
        value={selectedPurpose || null}
        onValueChange={(value) => {
          onPurposeChange(value ?? '')
        }}
        disabled={disabled}
      >
        <SelectTrigger aria-label="実行目的" className="min-w-0 flex-1">
          <SelectValue placeholder={defaultPurposeLabel} />
        </SelectTrigger>
        <SelectContent>
          {items.map(({ value, label }) => (
            <SelectItem
              key={value === null ? 'default' : `purpose-${value}`}
              value={value}
            >
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </label>
  )
}
