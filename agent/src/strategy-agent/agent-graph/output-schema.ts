import { isPlainObject } from '#strategy-agent/agent-graph/json'

// JSON Schema のフィールド断片。`type` は enum を使う場合など省略される
// こともあり、それ以外はユーザー定義の自由な語彙であるため、このモジュールは
// 中身の意味を一切解釈せず構造だけを組み替える。
export interface JsonSchemaObject {
  readonly type?: string
  readonly [key: string]: unknown
}

// フェーズ全体の `output` から組み立てるスキーマ。langchain の
// toolStrategy(JsonSchemaFormat) オーバーロードが期待する形に合わせているが、
// その型自体は langchain の公開エクスポートに含まれないため、import ではなく
// 構造的に互換な代替として定義している。
export interface ObjectJsonSchema {
  readonly type: 'object'
  readonly properties: Record<string, JsonSchemaObject>
  readonly required?: string[]
  readonly [key: string]: unknown
}

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((v) => typeof v === 'string')

// `output` の規約: フィールド名 -> スキーマ断片のマップ。`items` も
// オブジェクト配列要素では同じ規約 (フィールド名 -> スキーマ断片のマップ +
// 同階層の `required` キー) に従い、プリミティブ配列要素の場合はそれ自体が
// スキーマ断片そのもの (例: `{ type: string }`) になる。
//
// `required` は予約された同階層キー: フェーズの `output` (やオブジェクトの
// `items` マップ) は `required` という名前のフィールドを定義できない —
// プロパティにはならず `required` 配列に吸収されてしまう。
const toObjectSchema = (
  propsMap: Readonly<Record<string, unknown>>,
): ObjectJsonSchema => {
  const { required, ...fields } = propsMap
  const properties: Record<string, JsonSchemaObject> = {}
  for (const [name, fieldSchema] of Object.entries(fields)) {
    properties[name] = toFieldSchema(fieldSchema)
  }
  return {
    type: 'object',
    properties,
    ...(isStringArray(required) ? { required } : {}),
  }
}

const toFieldSchema = (fieldSchema: unknown): JsonSchemaObject => {
  if (!isPlainObject(fieldSchema)) return {}
  const { items, ...rest } = fieldSchema
  if (items === undefined) return rest
  return { ...rest, items: toItemsSchema(items) }
}

const toItemsSchema = (items: unknown): JsonSchemaObject => {
  if (!isPlainObject(items)) return {}
  // プリミティブ配列要素のスキーマ (例: `{ type: string }`) は自身の `type`
  // を持つが、オブジェクト配列要素のスキーマはフィールド名 -> スキーマ断片の
  // マップであり、`type` キー自体は持たない。
  return typeof items['type'] === 'string'
    ? toFieldSchema(items)
    : toObjectSchema(items)
}

export const buildOutputJsonSchema = (
  output: Readonly<Record<string, unknown>>,
): ObjectJsonSchema => toObjectSchema(output)
