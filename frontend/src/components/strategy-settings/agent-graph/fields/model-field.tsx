import { Input } from '#components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '#components/ui/select'
import type { components } from '#lib/api/schema.gen'

type AgentModel = components['schemas']['AgentModel']

interface ModelFieldProps {
  value: string
  onChange: (next: string) => void
  models: AgentModel[]
}

const TRIGGER_CLASS =
  'h-auto w-full max-w-sm justify-start rounded-none border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] py-1 font-mono text-[11.5px] text-[color:var(--color-text-primary)]'

export function ModelField({ value, onChange, models }: ModelFieldProps) {
  const label = (
    <span className="pt-1.5 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
      モデル
    </span>
  )

  // LiteLLM 未接続などで一覧が引けない場合は、既存の自由入力にフォールバックする
  if (models.length === 0) {
    return (
      <>
        {label}
        <Input
          aria-label="モデル"
          value={value}
          onChange={(e) => {
            onChange(e.target.value)
          }}
          className="h-auto w-full max-w-sm rounded-none border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] py-1 font-mono text-[11.5px] text-[color:var(--color-text-primary)]"
        />
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
    supports_web_search: false,
  }
  const options =
    value !== '' && !models.some((m) => m.id === value)
      ? [unknownCurrentValue, ...models]
      : models

  return (
    <>
      {label}
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger aria-label="モデル" className={TRIGGER_CLASS}>
          <SelectValue placeholder="モデルを選択" />
        </SelectTrigger>
        <SelectContent>
          {options.map((m) => (
            <SelectItem
              key={m.id}
              value={m.id}
              className="font-mono text-[11.5px]"
            >
              {m.id}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </>
  )
}
