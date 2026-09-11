import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']

interface StrategyFilterSelectProps {
  value: string | undefined
  onChange: (value: string | undefined) => void
}

/** 口座全体一覧ページで戦略による絞り込みに使う共通セレクタ */
export function StrategyFilterSelect({
  value,
  onChange,
}: StrategyFilterSelectProps) {
  const { data: strategies } = $api.useQuery('get', '/api/strategies')

  return (
    <StrategyFilterSelectView
      value={value}
      onChange={onChange}
      strategies={strategies ?? []}
    />
  )
}

interface StrategyFilterSelectViewProps {
  value: string | undefined
  onChange: (value: string | undefined) => void
  strategies: Strategy[]
}

export function StrategyFilterSelectView({
  value,
  onChange,
  strategies,
}: StrategyFilterSelectViewProps) {
  return (
    <select
      aria-label="戦略で絞り込み"
      value={value ?? ''}
      onChange={(e) => {
        onChange(e.target.value === '' ? undefined : e.target.value)
      }}
      className="h-9 border border-input bg-transparent px-3 font-mono text-xs"
    >
      <option value="">すべての戦略</option>
      {strategies.map((s) => (
        <option key={s.id} value={s.id}>
          {s.name}
        </option>
      ))}
    </select>
  )
}
