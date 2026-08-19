import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

// `output` の配列フィールド (`type: array`) だけを拾う。`required` は文字列配列であり
// isPlainObject を満たさないため、フィールドとして誤検出しない。
function arrayFieldNames(output: Record<string, unknown>): string[] {
  return Object.entries(output)
    .filter(([, schema]) => isPlainObject(schema) && schema['type'] === 'array')
    .map(([name]) => name)
}

export interface ForEachOption {
  /** "<phase_key>.<field>" の形。for_each にそのまま書き込む値 */
  value: string
  label: string
}

/**
 * index より前のフェーズが output で宣言している配列フィールドを列挙する。
 * ここに無い値は for_each に存在しない参照であり、選択肢として出さない。
 */
export function getForEachOptions(
  phases: readonly AgentGraphPhaseForm[],
  index: number,
): ForEachOption[] {
  return phases.slice(0, index).flatMap((phase) =>
    arrayFieldNames(phase.output).map((field) => ({
      value: `${phase.key}.${field}`,
      label: `${phase.label} → ${field}[] の要素ごと`,
    })),
  )
}

/**
 * for_each が指す配列フィールドの items から string 型の property 名を列挙する。
 * 参照先が存在しない、items がプリミティブ配列、または items の全 property が string 型
 * でない場合は空配列を返す。
 */
export function getLabelFieldOptions(
  phases: readonly AgentGraphPhaseForm[],
  forEach: string | undefined,
): string[] {
  if (forEach == null) return []
  const [phaseKey, field] = forEach.split('.')
  if (field == null) return []
  const phase = phases.find((p) => p.key === phaseKey)
  const fieldSchema = phase?.output[field]
  if (!isPlainObject(fieldSchema)) return []
  const items = fieldSchema['items']
  if (!isPlainObject(items)) return []
  return Object.entries(items)
    .filter(
      ([, schema]) => isPlainObject(schema) && schema['type'] === 'string',
    )
    .map(([name]) => name)
}
