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
  return (
    <label className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
      <span>実行目的</span>
      <select
        aria-label="実行目的"
        value={selectedPurpose}
        onChange={(e) => {
          onPurposeChange(e.target.value)
        }}
        disabled={disabled}
        className="min-w-0 flex-1 border border-border bg-background px-2 py-1.5 text-foreground outline-none disabled:opacity-60"
      >
        <option value="">既定の設定</option>
        {purposes.map((purpose) => (
          <option key={purpose} value={purpose}>
            {purpose}
          </option>
        ))}
      </select>
    </label>
  )
}
