import { isPlainObject } from '#strategy-agent/agent-graph/json'

// JSON Schema のフィールド断片。`type` は enum を使う場合など省略される
// こともあり、それ以外はユーザー定義の自由な語彙であるため、このモジュールは
// 中身の意味を一切解釈せず構造だけを組み替える。
interface JsonSchemaObject {
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

// enum など type を持たないスキーマ断片を保つため、値がすべてフィールド用
// スキーマ断片のマップである場合だけオブジェクトとして扱う。
const toFieldSchemaMap = (
  value: Record<string, unknown>,
): { fields: Record<string, unknown>; required: unknown } | undefined => {
  if (typeof value['type'] === 'string') return undefined

  const requiredIsMetadata =
    Object.hasOwn(value, 'required') && !isPlainObject(value['required'])
  const required = requiredIsMetadata ? value['required'] : undefined
  const fields = requiredIsMetadata
    ? Object.fromEntries(
        Object.entries(value).filter(([key]) => key !== 'required'),
      )
    : value
  const fieldSchemas = Object.values(fields)
  return (fieldSchemas.length > 0 || requiredIsMetadata) &&
    fieldSchemas.every(isPlainObject)
    ? { fields, required }
    : undefined
}

// `required` はフィールド名 -> スキーマ断片のマップを値として持つキー
// (フェーズの `output` 自身、または配列フィールドの `items`) と同階層に置く
// 予約キー。マップの値そのものではなく、そのマップを保持するキーの兄弟として
// 渡ってくるため、`fieldsMap` の外から引数で受け取る。
const toObjectSchema = (
  fieldsMap: Readonly<Record<string, unknown>>,
  required: unknown,
): ObjectJsonSchema => {
  const properties: Record<string, JsonSchemaObject> = {}
  for (const [name, fieldSchema] of Object.entries(fieldsMap)) {
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
  const fieldMap = toFieldSchemaMap(fieldSchema)
  if (fieldMap !== undefined) {
    return toObjectSchema(fieldMap.fields, fieldMap.required)
  }

  const { items, required, ...rest } = fieldSchema
  if (items === undefined) return rest
  return { ...rest, items: toItemsSchema(items, required) }
}

const toItemsSchema = (items: unknown, required: unknown): JsonSchemaObject => {
  if (!isPlainObject(items)) return {}
  const fieldMap = toFieldSchemaMap(items)
  return fieldMap === undefined
    ? toFieldSchema(items)
    : toObjectSchema(fieldMap.fields, required ?? fieldMap.required)
}

// toolStrategy は raw JSON Schema に description を自動補完しないため、
// 空文字を拒否する LLM プロバイダ向けに明示的に補う。
const STRUCTURED_OUTPUT_TOOL_DESCRIPTION =
  "Tool for extracting structured output from the model's response."

export const buildOutputJsonSchema = (
  output: Readonly<Record<string, unknown>>,
): ObjectJsonSchema => {
  const { required, ...fields } = output
  return {
    ...toObjectSchema(fields, required),
    description: STRUCTURED_OUTPUT_TOOL_DESCRIPTION,
  }
}
