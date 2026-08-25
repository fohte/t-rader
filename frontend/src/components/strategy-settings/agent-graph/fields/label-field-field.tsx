import { getLabelFieldOptions } from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '#components/ui/select'

interface LabelFieldFieldProps {
  phases: readonly AgentGraphPhaseForm[]
  forEach: string | undefined
  value: string | undefined
  onChange: (next: string | undefined) => void
}

export function LabelFieldField({
  phases,
  forEach,
  value,
  onChange,
}: LabelFieldFieldProps) {
  const options = getLabelFieldOptions(phases, forEach)

  return (
    <>
      <span className="pt-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        ノード名
      </span>
      <Select
        value={value}
        onValueChange={(next) => {
          // Base UI の Select.onValueChange は null を渡しうるため undefined に正規化する
          onChange(next ?? undefined)
        }}
        disabled={options.length === 0}
      >
        <SelectTrigger
          aria-label="ノード名"
          className="h-auto w-full max-w-[160px] rounded-none border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] py-1 font-mono text-[11.5px] text-[color:var(--color-text-primary)]"
        >
          <SelectValue placeholder="(選択肢なし)" />
        </SelectTrigger>
        <SelectContent>
          {options.map((field) => (
            <SelectItem key={field} value={field}>
              {field}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </>
  )
}
