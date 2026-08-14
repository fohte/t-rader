import { isPlainObject } from '#strategy-agent/agent-graph/json'

// A JSON Schema field fragment. `type` is optional since a field may use
// `enum` instead, and everything else is untyped user vocabulary (see
// CLAUDE.md's "セマンティック型をソースコードにハードコードしない" policy)
// that this module reshapes without ever inspecting its meaning.
export interface JsonSchemaObject {
  readonly type?: string
  readonly [key: string]: unknown
}

// The schema built for a whole phase's `output`, matching the shape
// langchain's toolStrategy(JsonSchemaFormat) overload expects (that type
// itself isn't part of langchain's public export surface, so this is a
// structurally-compatible stand-in rather than an import).
export interface ObjectJsonSchema {
  readonly type: 'object'
  readonly properties: Record<string, JsonSchemaObject>
  readonly required?: string[]
  readonly [key: string]: unknown
}

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((v) => typeof v === 'string')

// `output`'s convention: a field-name -> schema-fragment map. `items`
// follows the same convention for object array elements (a field-name ->
// schema-fragment map, with a sibling `required` key), or is itself a plain
// schema fragment for primitive array elements (e.g. `{ type: string }`).
//
// `required` is a reserved sibling key: a phase's `output` (or an object
// `items` map) cannot define its own field literally named `required` — it
// will be swallowed into the schema's `required` array instead of becoming
// a property.
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
  // A primitive items schema (e.g. `{ type: string }`) carries its own
  // `type`; an object items schema is a field-name -> schema-fragment map
  // with no `type` key of its own.
  return typeof items['type'] === 'string'
    ? toFieldSchema(items)
    : toObjectSchema(items)
}

export const buildOutputJsonSchema = (
  output: Readonly<Record<string, unknown>>,
): ObjectJsonSchema => toObjectSchema(output)
