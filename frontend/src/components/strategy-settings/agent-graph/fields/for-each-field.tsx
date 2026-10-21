import { Chip } from '@fohte/ui/chip'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

import { getForEachOptions } from '#components/strategy-settings/agent-graph/output-fields'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

// for_each 未設定 (1 実行につき 1 回) を表す sentinel。このセレクトが書き込む値は必ず
// "<phase_key>.<field>" の形でドットを含むため衝突しないが、YAML ビューで直接
// `for_each: __once__` のようなドットの無い値を書いた場合はこの前提が崩れる。
const ONCE_VALUE = '__once__'

interface ForEachFieldProps {
  phases: readonly AgentGraphPhaseForm[]
  index: number
  value: string | undefined
  onChange: (next: string | undefined) => void
  /** Storybook でセレクトを開いた状態を描画するためのフラグ */
  selectDefaultOpen?: boolean
}

export function ForEachField({
  phases,
  index,
  value,
  onChange,
  selectDefaultOpen,
}: ForEachFieldProps) {
  const options = getForEachOptions(phases, index)
  const items = [{ value: ONCE_VALUE, label: '1 実行につき 1 回' }, ...options]

  return (
    <>
      <Chip>実行回数</Chip>
      <Select
        items={items}
        value={value ?? ONCE_VALUE}
        onValueChange={(next) => {
          // Base UI の Select.onValueChange は null を渡しうるため、sentinel と同様に
          // 未設定 (undefined) として扱う
          onChange(next == null || next === ONCE_VALUE ? undefined : next)
        }}
        defaultOpen={selectDefaultOpen}
      >
        <div className="w-full max-w-sm font-mono">
          <SelectTrigger aria-label="実行回数" className="w-full justify-start">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={ONCE_VALUE}>
              <span className="font-mono">1 実行につき 1 回</span>
            </SelectItem>
            {options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                <span className="font-mono">{option.label}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </div>
      </Select>
    </>
  )
}
