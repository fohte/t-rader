import { Result } from 'neverthrow'
import { parseDocument } from 'yaml'

import type { components } from '#lib/api/schema.gen'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']
type WidgetUniverse = components['schemas']['WidgetUniverseRequest']
type WidgetVisualization = components['schemas']['WidgetVisualizationRequest']
export type SupportedWidgetVisualization = Extract<
  WidgetVisualization,
  { type: 'treemap' | 'toplist' | 'table' }
>

export type WidgetDefinitionResult =
  | {
      ok: true
      request: WidgetQueryRequest
      viz: WidgetVisualization
      asOf?: string
    }
  | { ok: false; message: string; path?: string }

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

function isWidgetQuery(value: unknown): boolean {
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

function isDisplay(value: unknown): boolean {
  return (
    isRecord(value) &&
    isOptionalString(value.label) &&
    isOptionalString(value.format) &&
    hasOnlyKeys(value, ['label', 'format'])
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
  const unsupportedKey = Object.keys(value).find(
    (key) => !['queries', 'formulas', 'display', 'viz', 'time'].includes(key),
  )
  if (unsupportedKey != null) {
    return {
      ok: false,
      path: unsupportedKey,
      message: '未対応の項目です。',
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
  if (
    value.display !== undefined &&
    (!isRecord(value.display) || !Object.values(value.display).every(isDisplay))
  ) {
    return { ok: false, message: 'display は表示設定のマップにしてください。' }
  }
  if (!isVisualization(value.viz)) {
    return { ok: false, message: 'viz.type は文字列で指定してください。' }
  }
  if (value.time !== undefined && !isRecord(value.time)) {
    return { ok: false, message: 'time はオブジェクトにしてください。' }
  }
  const time = isRecord(value.time) ? value.time : undefined
  const unsupportedTimeKey =
    time == null
      ? undefined
      : Object.keys(time).find((key) => !['as_of', 'range'].includes(key))
  if (unsupportedTimeKey != null) {
    return {
      ok: false,
      path: `time.${unsupportedTimeKey}`,
      message: '未対応の項目です。',
    }
  }
  if (time !== undefined && !isOptionalString(time.as_of)) {
    return { ok: false, message: 'time.as_of は文字列で指定してください。' }
  }
  if (time !== undefined && Object.hasOwn(time, 'range')) {
    return {
      ok: false,
      path: 'time.range',
      message: 'time.range は現在のウィジェットでは使えません。',
    }
  }
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- enum 値と範囲は API に検証させるため、生成型にない値も保持する
  const request = value as unknown as WidgetQueryRequest
  const viz = request.viz
  const asOf = typeof time?.as_of === 'string' ? time.as_of : undefined
  return { ok: true, request, viz, asOf }
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
  return (
    viz.type === 'treemap' || viz.type === 'toplist' || viz.type === 'table'
  )
}
