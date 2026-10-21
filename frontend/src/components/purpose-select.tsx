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
  const selectablePurposes =
    selectedPurpose === '' || purposes.includes(selectedPurpose)
      ? purposes
      : [selectedPurpose, ...purposes]
  const items = [
    { value: null, label: '既定の設定' },
    ...selectablePurposes.map((purpose) => ({
      value: purpose,
      label: purpose,
    })),
  ]

  return (
    <label className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
      <span>実行目的</span>
      <Select
        items={items}
        value={selectedPurpose || null}
        onValueChange={(value) => {
          onPurposeChange(value ?? '')
        }}
        disabled={disabled}
      >
        <SelectTrigger aria-label="実行目的" className="min-w-0 flex-1">
          <SelectValue placeholder="既定の設定" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={null}>既定の設定</SelectItem>
          {selectablePurposes.map((purpose) => (
            <SelectItem key={purpose} value={purpose}>
              {purpose}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </label>
  )
}
