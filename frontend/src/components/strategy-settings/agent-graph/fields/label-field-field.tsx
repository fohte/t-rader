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
      <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
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
          className="h-auto w-full max-w-40 rounded-none border-border bg-background py-1 font-mono text-xs text-foreground"
        >
          {/* schema 未定義等で value が options に無い場合、Base UI のデフォルト表示は
              value を生表示してしまうため、options に無い値は空表示にする */}
          <SelectValue>
            {(v: string | null) =>
              v == null ? '(選択肢なし)' : options.includes(v) ? v : null
            }
          </SelectValue>
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
