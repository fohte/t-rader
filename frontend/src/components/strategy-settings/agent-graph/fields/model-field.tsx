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

export function ModelField({ value, onChange, models }: ModelFieldProps) {
  const label = (
    <span className="pt-1.5 font-mono text-2xs text-muted-foreground">
      モデル
    </span>
  )

  // LLM ゲートウェイ未接続などで一覧が引けない場合は、既存の自由入力にフォールバックする
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
          className="h-auto w-full max-w-sm rounded-none border-border bg-background py-1 font-mono text-2xs text-foreground"
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
      <Select
        value={value}
        onValueChange={(next) => {
          // Base UI の Select.onValueChange は null を渡しうるが、この Select は常にいずれかの
          // モデルが選択された状態のため null は無視する
          if (next != null) onChange(next)
        }}
      >
        <SelectTrigger
          aria-label="モデル"
          className="h-auto w-full max-w-sm justify-start rounded-none border-border bg-background py-1 font-mono text-2xs text-foreground"
        >
          <SelectValue placeholder="モデルを選択" />
        </SelectTrigger>
        <SelectContent>
          {options.map((m) => (
            <SelectItem key={m.id} value={m.id} className="font-mono text-2xs">
              {m.id}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </>
  )
}
