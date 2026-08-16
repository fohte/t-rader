import {
  isMap,
  isNode,
  isScalar,
  isSeq,
  LineCounter,
  parseDocument,
} from 'yaml'

export interface OutputSchemaIssue {
  message: string
  line: number
  column: number
}

export interface OutputSchemaCheckResult {
  /** YAML 構文が有効なら常に値を持つ (structural な issue があっても commit 対象になる)。構文エラーのときのみ null */
  output: Record<string, unknown> | null
  issues: OutputSchemaIssue[]
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
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

// output-schema.ts の isStringArray と同じ定義
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
      checkRequired(lineCounter, value, `${label}.required`, issues)
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
  const typePair = node.items.find((p) => scalarKey(p) === 'type')
  const typeValue: unknown = typePair?.value
  const isPrimitiveItems =
    isScalar(typeValue) && typeof typeValue.value === 'string'
  if (isPrimitiveItems) {
    checkFieldSchema(lineCounter, node, label, issues)
  } else {
    // オブジェクト配列要素のフィールドマップ。output-schema.ts の toObjectSchema は
    // このマップから `required` を一切 strip しないため、ここでの `required` は
    // 特別扱いされずただのフィールド名として扱われる (トップレベルとの非対称性)。
    checkFieldsMap(lineCounter, node, false, issues)
  }
}

// フィールド名 -> フィールドスキーマのマップ (output 自身、または object 配列の items) を検査する
function checkFieldsMap(
  lineCounter: LineCounter,
  map: { items: { key: unknown; value: unknown }[] },
  reserveRequired: boolean,
  issues: OutputSchemaIssue[],
): void {
  for (const pair of map.items) {
    const key = scalarKey(pair)
    const value = pair.value
    if (reserveRequired && key === 'required') {
      checkRequired(lineCounter, value, 'required', issues)
      continue
    }
    checkFieldSchema(lineCounter, value, key ?? '(フィールド)', issues)
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
