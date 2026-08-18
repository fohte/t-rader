import {
  type Document,
  isMap,
  isSeq,
  parseDocument,
  Scalar,
  YAMLSeq,
} from 'yaml'

import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

export function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === 'string')
}

// 省略 (undefined) と明示的な空配列を区別する。存在するが不正な形の場合は
// skills と同じく [] にフォールバックする (エージェント側が実行時に弾く)
function toOptionalStringArray(v: unknown): string[] | undefined {
  if (v === undefined) return undefined
  return isStringArray(v) ? v : []
}

function toPhaseForm(v: unknown): AgentGraphPhaseForm | null {
  if (!isRecord(v)) return null
  const { key, label, model, prompt } = v
  if (
    typeof key !== 'string' ||
    typeof label !== 'string' ||
    typeof model !== 'string' ||
    typeof prompt !== 'string'
  ) {
    return null
  }
  return {
    key,
    label,
    model,
    prompt,
    forEach: typeof v['for_each'] === 'string' ? v['for_each'] : undefined,
    labelField:
      typeof v['label_field'] === 'string' ? v['label_field'] : undefined,
    maxParallel:
      typeof v['max_parallel'] === 'number' ? v['max_parallel'] : undefined,
    skills: isStringArray(v['skills']) ? v['skills'] : [],
    tools: toOptionalStringArray(v['tools']),
    output: isRecord(v['output']) ? v['output'] : {},
  }
}

/**
 * agent_graph の YAML から編集対象のフェーズ一覧を取り出す。
 * 構文エラー、`phases` がトップレベルに存在し配列であることを満たさない場合、または
 * 各フェーズが key/label/model/prompt を string で持たない (フォームで扱えない形の)
 * 場合は null を返す。呼び出し側はこれを「フォーム表示不可、YAML ビューにフォールバック」
 * の合図として使う。
 *
 * 空文字列 (フェーズ分割 off) は空配列を返す。
 */
export function parseAgentGraphPhases(
  yamlText: string,
): AgentGraphPhaseForm[] | null {
  if (yamlText.trim() === '') return []
  const doc = parseDocument(yamlText)
  if (doc.errors.length > 0) return null
  const parsed: unknown = doc.toJS()
  if (!isRecord(parsed)) return null
  const phases = parsed['phases']
  if (!Array.isArray(phases)) return null

  const result: AgentGraphPhaseForm[] = []
  for (const p of phases) {
    const phase = toPhaseForm(p)
    if (phase == null) return null
    result.push(phase)
  }
  return result
}

function getPhasesSeq(doc: Document): YAMLSeq {
  const seq: unknown = doc.get('phases', true)
  if (isSeq(seq)) return seq
  doc.set('phases', [])
  const created: unknown = doc.get('phases', true)
  // `doc.set` 直後の get は必ず YAMLSeq を返すが、Document.get() の型が
  // `unknown` のため型的には保証できない。フォールバックとして空の
  // YAMLSeq を返し、型アサーションを避ける。
  return isSeq(created) ? created : new YAMLSeq()
}

function withDocument(
  yamlText: string,
  mutate: (doc: Document, seq: YAMLSeq) => void,
): string {
  const doc = parseDocument(yamlText.trim() === '' ? 'phases: []' : yamlText)
  mutate(doc, getPhasesSeq(doc))
  return doc.toString()
}

// 改行を含む値は `|-` (block literal) にしないと、yaml の default stringify が
// 折り返し規則で改行を潰した plain scalar を吐く (yaml 2.9 で確認済み)。
function setStringField(
  doc: Document,
  map: unknown,
  field: string,
  value: string,
) {
  if (!isMap(map)) return
  const node = doc.createNode(value)
  if (node instanceof Scalar && value.includes('\n')) {
    node.type = Scalar.BLOCK_LITERAL
  }
  map.set(field, node)
}

export function setPhaseField(
  yamlText: string,
  index: number,
  field: 'model' | 'prompt',
  value: string,
): string {
  return withDocument(yamlText, (doc, seq) => {
    setStringField(doc, seq.items[index], field, value)
  })
}

export function setPhaseOutput(
  yamlText: string,
  index: number,
  output: Record<string, unknown>,
): string {
  return withDocument(yamlText, (doc, seq) => {
    const map = seq.items[index]
    if (!isMap(map)) return
    map.set('output', doc.createNode(output))
  })
}

/**
 * tools/skills のような配列フィールドを書き換える。`value` が `undefined` の場合は
 * キー自体を削除する (tools の「省略 = 全 tool 使用可」を表現するため)。`[]` は
 * 明示的な空配列としてそのまま書き込まれ、undefined とは区別される。
 */
export function setPhaseArrayField(
  yamlText: string,
  index: number,
  field: 'tools' | 'skills',
  value: string[] | undefined,
): string {
  return withDocument(yamlText, (doc, seq) => {
    const map = seq.items[index]
    if (!isMap(map)) return
    if (value === undefined) {
      map.delete(field)
      return
    }
    map.set(field, doc.createNode(value))
  })
}

function nextPhaseKey(existing: string[]): string {
  let n = existing.length + 1
  while (existing.includes(`phase-${String(n)}`)) n++
  return `phase-${String(n)}`
}

export function addPhase(yamlText: string): string {
  return withDocument(yamlText, (doc, seq) => {
    const keys = seq.items.filter(isMap).map((m) => String(m.get('key')))
    seq.add(
      doc.createNode({
        key: nextPhaseKey(keys),
        label: '新しいフェーズ',
        model: '',
        prompt: '',
      }),
    )
  })
}

export function removePhase(yamlText: string, index: number): string {
  return withDocument(yamlText, (_doc, seq) => {
    seq.items.splice(index, 1)
  })
}

export function movePhase(
  yamlText: string,
  index: number,
  direction: 'up' | 'down',
): string {
  return withDocument(yamlText, (_doc, seq) => {
    const target = direction === 'up' ? index - 1 : index + 1
    if (target < 0 || target >= seq.items.length) return
    const items = seq.items
    ;[items[index], items[target]] = [items[target], items[index]]
  })
}

/** フェーズ分割トグルを空の状態から on にするときの初期テンプレート。 */
export const DEFAULT_AGENT_GRAPH_YAML = `phases:
  - key: phase-1
    label: 新しいフェーズ
    model: ""
    prompt: ""
`
