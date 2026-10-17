import {
  isMap,
  isNode,
  isScalar,
  isSeq,
  LineCounter,
  parseDocument,
} from 'yaml'

import { isRecord } from '#components/strategy-settings/agent-graph/document'

// agent/src/strategy-agent/agent-graph/output-schema.ts (buildOutputJsonSchema) が
// 実際に受理する構造を手で再現した検証。frontend と agent は別 pnpm workspace で
// 直接 import できないため、output-schema.ts の構造規則が変わったらここも合わせて
// 直す必要がある (機械的な整合チェックは無い)。
interface OutputSchemaIssue {
  message: string
  line: number
  column: number
}

export interface OutputSchemaCheckResult {
  /** YAML 構文が有効なら常に値を持つ (structural な issue があっても commit 対象になる)。構文エラーのときのみ null */
  output: Record<string, unknown> | null
  issues: OutputSchemaIssue[]
}

function scalarKey(pair: { key: unknown }): string | null {
  return isScalar(pair.key) && typeof pair.key.value === 'string'
    ? pair.key.value
    : null
}

function pushIssue(
  lineCounter: LineCounter,
  node: unknown,
  message: string,
  issues: OutputSchemaIssue[],
): void {
  const offset = isNode(node) ? (node.range?.[0] ?? 0) : 0
  const { line, col } = lineCounter.linePos(offset)
  issues.push({ message, line, column: col })
}

// agent/src/strategy-agent/agent-graph/output-schema.ts の isStringArray と同じ定義
function isStringArraySeq(node: unknown): boolean {
  return (
    isSeq(node) &&
    node.items.every((item) => isScalar(item) && typeof item.value === 'string')
  )
}

function checkRequired(
  lineCounter: LineCounter,
  node: unknown,
  label: string,
  issues: OutputSchemaIssue[],
): void {
  if (!isStringArraySeq(node)) {
    pushIssue(
      lineCounter,
      node,
      `${label} は文字列の配列である必要があります`,
      issues,
    )
  }
}

function hasStringType(node: {
  items: { key: unknown; value: unknown }[]
}): boolean {
  const typePair = node.items.find((p) => scalarKey(p) === 'type')
  const typeValue: unknown = typePair?.value
  return isScalar(typeValue) && typeof typeValue.value === 'string'
}

// agent 側が type-less なフィールドマップとして扱う条件を再現する
function isFieldSchemaMap(
  node: { items: { key: unknown; value: unknown }[] },
  reserveRequired = true,
): boolean {
  if (hasStringType(node)) return false

  const requiredPair = node.items.find((pair) => scalarKey(pair) === 'required')
  const requiredIsMetadata =
    reserveRequired && requiredPair !== undefined && !isMap(requiredPair.value)
  const fieldPairs = node.items.filter(
    (pair) => !(requiredIsMetadata && pair === requiredPair),
  )
  return (
    (fieldPairs.length > 0 || requiredIsMetadata) &&
    fieldPairs.every((pair) => isMap(pair.value))
  )
}

// フィールド 1 件のスキーマ断片 (type/description/enum/items/required など) を検査する
function checkFieldSchema(
  lineCounter: LineCounter,
  node: unknown,
  label: string,
  issues: OutputSchemaIssue[],
): void {
  if (!isMap(node)) {
    pushIssue(
      lineCounter,
      node,
      `${label} はオブジェクトである必要があります`,
      issues,
    )
    return
  }

  if (isFieldSchemaMap(node)) {
    const requiredPair = node.items.find(
      (pair) => scalarKey(pair) === 'required',
    )
    const requiredIsMetadata =
      requiredPair !== undefined && !isMap(requiredPair.value)
    if (requiredIsMetadata) {
      checkRequired(
        lineCounter,
        requiredPair.value,
        `${label}.required`,
        issues,
      )
    }
    checkFieldsMap(lineCounter, node, requiredIsMetadata, issues, label)
    return
  }

  // 配列フィールドの required は、items がフィールドマップの場合にだけ有効
  const itemsPair = node.items.find((p) => scalarKey(p) === 'items')
  const itemsValue: unknown = itemsPair?.value
  const requiredIsEffective =
    isMap(itemsValue) && isFieldSchemaMap(itemsValue, false)

  for (const pair of node.items) {
    const key = scalarKey(pair)
    const value: unknown = pair.value
    if (key === 'enum') {
      if (!isSeq(value)) {
        pushIssue(
          lineCounter,
          value,
          `${label}.enum は配列である必要があります`,
          issues,
        )
      }
    } else if (key === 'items') {
      checkItems(lineCounter, value, label, issues)
    } else if (key === 'required') {
      if (requiredIsEffective) {
        checkRequired(lineCounter, value, `${label}.required`, issues)
      } else {
        pushIssue(
          lineCounter,
          value,
          `${label}.required は items がオブジェクト配列の場合のみ有効です (ここでは無視されます)`,
          issues,
        )
      }
    }
  }
}

function checkItems(
  lineCounter: LineCounter,
  node: unknown,
  fieldLabel: string,
  issues: OutputSchemaIssue[],
): void {
  const label = `${fieldLabel}.items`
  if (!isMap(node)) {
    pushIssue(
      lineCounter,
      node,
      `${label} はオブジェクトである必要があります`,
      issues,
    )
    return
  }
  if (hasStringType(node)) {
    checkFieldSchema(lineCounter, node, label, issues)
  } else if (isFieldSchemaMap(node, false)) {
    checkFieldsMap(lineCounter, node, false, issues, label)
  } else {
    checkFieldSchema(lineCounter, node, label, issues)
  }
}

// フィールド名 -> フィールドスキーマのマップ (output 自身、または object 配列の items) を検査する
function checkFieldsMap(
  lineCounter: LineCounter,
  map: { items: { key: unknown; value: unknown }[] },
  reserveRequired: boolean,
  issues: OutputSchemaIssue[],
  label = '',
): void {
  for (const pair of map.items) {
    const key = scalarKey(pair)
    const value = pair.value
    const fieldLabel =
      label === ''
        ? (key ?? '(フィールド)')
        : `${label}.${key ?? '(フィールド)'}`
    if (reserveRequired && key === 'required') {
      checkRequired(
        lineCounter,
        value,
        label === '' ? 'required' : `${label}.required`,
        issues,
      )
      continue
    }
    checkFieldSchema(lineCounter, value, fieldLabel, issues)
  }
}

export function checkOutputSchemaText(text: string): OutputSchemaCheckResult {
  const lineCounter = new LineCounter()
  const source = text.trim() === '' ? '{}' : text
  const doc = parseDocument(source, { lineCounter })

  if (doc.errors.length > 0) {
    const err = doc.errors[0]
    if (err == null) return { output: null, issues: [] }
    const pos = err.linePos?.[0] ?? lineCounter.linePos(err.pos[0])
    return {
      output: null,
      issues: [{ message: err.message, line: pos.line, column: pos.col }],
    }
  }

  const issues: OutputSchemaIssue[] = []
  const root: unknown = doc.contents
  if (!isMap(root)) {
    pushIssue(
      lineCounter,
      root,
      'output はフィールド名 → スキーマのマップである必要があります',
      issues,
    )
    return { output: {}, issues }
  }

  checkFieldsMap(lineCounter, root, true, issues)

  const parsed: unknown = doc.toJS()
  return { output: isRecord(parsed) ? parsed : {}, issues }
}
