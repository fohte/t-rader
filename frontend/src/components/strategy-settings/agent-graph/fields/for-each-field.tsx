import { getForEachOptions } from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '#components/ui/select'

// for_each 未設定 (1 実行につき 1 回) を表す sentinel。このセレクトが書き込む値は必ず
// "<phase_key>.<field>" の形でドットを含むため衝突しないが、YAML ビューで直接
// `for_each: __once__` のようなドットの無い値を書いた場合はこの前提が崩れる。
const ONCE_VALUE = '__once__'

interface ForEachFieldProps {
  phases: readonly AgentGraphPhaseForm[]
  index: number
  value: string | undefined
  onChange: (next: string | undefined) => void
}

export function ForEachField({
  phases,
  index,
  value,
  onChange,
}: ForEachFieldProps) {
  const options = getForEachOptions(phases, index)

  return (
    <>
      <span className="pt-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        実行回数
      </span>
      <Select
        value={value ?? ONCE_VALUE}
        onValueChange={(next) => {
          onChange(next === ONCE_VALUE ? undefined : next)
        }}
      >
        <SelectTrigger
          aria-label="実行回数"
          className="h-auto w-full max-w-sm rounded-none border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] py-1 font-mono text-[11.5px] text-[color:var(--color-text-primary)]"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={ONCE_VALUE}>1 実行につき 1 回</SelectItem>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </>
  )
}
