import { Chip } from '@fohte/ui/chip'
import { Input } from '@fohte/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

import type { components } from '#lib/api/schema.gen'

type AgentModel = components['schemas']['AgentModel']

interface ModelFieldProps {
  value: string
  onChange: (next: string) => void
  models: AgentModel[]
  /** Storybook で候補 popup を開いた状態を描画するための初期値 */
  defaultOpen?: boolean
}

export function ModelField({
  value,
  onChange,
  models,
  defaultOpen,
}: ModelFieldProps) {
  const label = <Chip>モデル</Chip>

  if (models.length === 0) {
    return (
      <>
        {label}
        <div className="w-full max-w-sm font-mono">
          <Input
            aria-label="モデル"
            value={value}
            onChange={(e) => {
              onChange(e.target.value)
            }}
          />
        </div>
      </>
    )
  }

  // 既に設定されている値が一覧に無くても (typo・非推奨モデルなど) 選択肢から消さない
  const unknownCurrentValue: AgentModel = {
    id: value,
    providers: [],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
  }
  const options =
    value !== '' && !models.some((m) => m.id === value)
      ? [unknownCurrentValue, ...models]
      : models

  return (
    <>
      {label}
      <Select
        value={value}
        defaultOpen={defaultOpen}
        onValueChange={(next) => {
          // Base UI の Select.onValueChange は null を渡しうるが、この Select は常にいずれかの
          // モデルが選択された状態のため null は無視する
          if (next != null) onChange(next)
        }}
      >
        <div className="w-full max-w-sm font-mono">
          <SelectTrigger aria-label="モデル" className="w-full justify-start">
            <SelectValue placeholder="モデルを選択" />
          </SelectTrigger>
          <SelectContent>
            {options.map((m) => (
              <SelectItem key={m.id} value={m.id}>
                <span className="font-mono">{m.id}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </div>
      </Select>
    </>
  )
}
