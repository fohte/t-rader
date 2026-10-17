import { Result } from 'neverthrow'
import { parseDocument } from 'yaml'

import type { components } from '#lib/api/schema.gen'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']
type WidgetQueryRequestDefinition =
  components['schemas']['WidgetQueryRequestDefinition']
type WidgetUniverse = components['schemas']['WidgetUniverseRequest']
type WidgetVisualization = components['schemas']['WidgetVisualizationRequest']
type SupportedWidgetVisualization = Extract<
  WidgetVisualization,
  { type: 'treemap' | 'toplist' }
>

export type WidgetDefinitionResult =
  | {
      ok: true
      request: WidgetQueryRequest
      viz: WidgetVisualization
      asOf: string
    }
  | { ok: false; message: string }

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  return Object.keys(value).every((key) => keys.includes(key))
}

function isOptionalString(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'string'
}

function isUniverse(value: unknown): value is WidgetUniverse {
  if (!isRecord(value)) return false
  if (typeof value.group === 'string' && hasOnlyKeys(value, ['group'])) {
    return true
  }
  return (
    Array.isArray(value.stocks) &&
    value.stocks.every((stock) => typeof stock === 'string') &&
    hasOnlyKeys(value, ['stocks'])
  )
}

function isWidgetQuery(value: unknown): value is WidgetQueryRequestDefinition {
  return (
    isRecord(value) &&
    typeof value.metric === 'string' &&
    typeof value.by === 'string' &&
    isOptionalString(value.period) &&
    isOptionalString(value.axis) &&
    isOptionalString(value.indicator) &&
    (value.universe == null || isUniverse(value.universe)) &&
    isOptionalString(value.agg)
  )
}

function isVisualization(value: unknown): boolean {
  return isRecord(value) && typeof value.type === 'string'
}

function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : 'YAML を解析できませんでした。'
}

export function parseWidgetQueryRequest(
  value: unknown,
): WidgetDefinitionResult {
  if (!isRecord(value)) {
    return {
      ok: false,
      message: 'ウィジェット定義は YAML オブジェクトにしてください。',
    }
  }
  if (!isRecord(value.queries)) {
    return { ok: false, message: 'queries はオブジェクトにしてください。' }
  }
  for (const [name, query] of Object.entries(value.queries)) {
    if (!isWidgetQuery(query)) {
      return {
        ok: false,
        message: `queries.${name} の形式が正しくありません。`,
      }
    }
  }
  if (
    value.formulas !== undefined &&
    (!isRecord(value.formulas) ||
      !Object.values(value.formulas).every(
        (formula) => typeof formula === 'string',
      ))
  ) {
    return { ok: false, message: 'formulas は文字列のマップにしてください。' }
  }
  if (!isVisualization(value.viz)) {
    return { ok: false, message: 'viz.type は文字列で指定してください。' }
  }
  if (!isRecord(value.time) || typeof value.time.as_of !== 'string') {
    return { ok: false, message: 'time.as_of は文字列で指定してください。' }
  }
  if (Object.hasOwn(value.time, 'range')) {
    return {
      ok: false,
      message: 'time.range は現在のウィジェットでは使えません。',
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- enum 値と範囲は API に検証させるため、生成型にない値も保持する
  const request = value as unknown as WidgetQueryRequest
  const viz = request.viz
  return { ok: true, request, viz, asOf: value.time.as_of }
}

export function parseWidgetDefinition(source: string): WidgetDefinitionResult {
  const document = parseDocument(source, { uniqueKeys: true })
  const parseError = document.errors[0]
  if (parseError != null) {
    return {
      ok: false,
      message: `YAML を解析できません: ${parseError.message}`,
    }
  }

  const parsed = Result.fromThrowable(
    (): unknown => document.toJS({ maxAliasCount: 10 }),
    errorMessage,
  )()
  if (parsed.isErr()) return { ok: false, message: parsed.error }
  return parseWidgetQueryRequest(parsed.value)
}

export function isSupportedWidgetVisualization(
  viz: WidgetVisualization,
): viz is SupportedWidgetVisualization {
  return viz.type === 'treemap' || viz.type === 'toplist'
}
