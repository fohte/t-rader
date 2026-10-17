import { Result } from 'neverthrow'
import { parseDocument } from 'yaml'

import type { components } from '#lib/api/schema.gen'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']
type WidgetQueryRequestDefinition =
  components['schemas']['WidgetQueryRequestDefinition']
type WidgetMetric = components['schemas']['WidgetMetricRequest']
type WidgetPeriod = components['schemas']['WidgetPeriodRequest']
type WidgetBy = components['schemas']['WidgetByRequest']
type WidgetAggregation = components['schemas']['WidgetAggregationRequest']
type WidgetUniverse = components['schemas']['WidgetUniverseRequest']
type WidgetVisualization = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' | 'toplist' }
>
type WidgetTime = components['schemas']['WidgetTimeRequest']

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  return Object.keys(value).every((key) => keys.includes(key))
}

function isOneOf<const Value extends string>(
  value: unknown,
  values: readonly Value[],
): value is Value {
  return typeof value === 'string' && values.some((item) => item === value)
}

function isCalendarDate(value: unknown): value is string {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return false
  }

  const date = new Date(`${value}T00:00:00.000Z`)
  return (
    Number.isFinite(date.valueOf()) && date.toISOString().slice(0, 10) === value
  )
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
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'metric',
      'period',
      'by',
      'axis',
      'indicator',
      'universe',
      'agg',
    ]) ||
    !isOneOf(value.metric, [
      'change',
      'close',
      'turnover',
      'mkt_cap',
      'per',
      'pbr',
      'indicator',
    ] satisfies readonly WidgetMetric[]) ||
    !isOneOf(value.by, ['none', 'stock', 'group'] satisfies readonly WidgetBy[])
  ) {
    return false
  }

  if (
    'period' in value &&
    value.period !== null &&
    !isOneOf(value.period, [
      '1d',
      '5d',
      '20d',
      '60d',
    ] satisfies readonly WidgetPeriod[])
  ) {
    return false
  }
  if (
    'axis' in value &&
    value.axis !== null &&
    typeof value.axis !== 'string'
  ) {
    return false
  }
  if (
    'indicator' in value &&
    value.indicator !== null &&
    typeof value.indicator !== 'string'
  ) {
    return false
  }
  if (
    'universe' in value &&
    value.universe !== null &&
    !isUniverse(value.universe)
  ) {
    return false
  }
  if (
    'agg' in value &&
    value.agg !== null &&
    !isOneOf(value.agg, [
      'mkt_cap_weighted',
      'mean',
      'sum',
      'count',
    ] satisfies readonly WidgetAggregation[])
  ) {
    return false
  }
  return true
}

function isVisualization(value: unknown): value is WidgetVisualization {
  if (!isRecord(value)) return false
  if (
    value.type === 'treemap' &&
    hasOnlyKeys(value, ['type', 'size', 'color'])
  ) {
    return typeof value.size === 'string' && typeof value.color === 'string'
  }
  return (
    value.type === 'toplist' &&
    hasOnlyKeys(value, ['type', 'value', 'order', 'limit']) &&
    typeof value.value === 'string' &&
    isOneOf(value.order, ['asc', 'desc']) &&
    typeof value.limit === 'number' &&
    Number.isInteger(value.limit) &&
    value.limit >= 0 &&
    value.limit <= 4_294_967_295
  )
}

function isWidgetTime(value: unknown): value is WidgetTime & { as_of: string } {
  return (
    isRecord(value) &&
    hasOnlyKeys(value, ['as_of', 'range']) &&
    isCalendarDate(value.as_of) &&
    (!('range' in value) ||
      value.range === null ||
      typeof value.range === 'string')
  )
}

function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : 'YAML を解析できませんでした。'
}

export function parseWidgetDefinition(source: string):
  | {
      ok: true
      request: WidgetQueryRequest
      viz: WidgetVisualization
      asOf: string
    }
  | { ok: false; message: string } {
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
  if (!isRecord(parsed.value)) {
    return {
      ok: false,
      message: 'ウィジェット定義は YAML オブジェクトにしてください。',
    }
  }
  const value = parsed.value
  if (
    !hasOnlyKeys(value, ['queries', 'formulas', 'viz', 'time']) ||
    !isRecord(value.queries) ||
    !isVisualization(value.viz) ||
    !isWidgetTime(value.time)
  ) {
    return {
      ok: false,
      message: 'queries、対応する viz、time.as_of を含む定義が必要です。',
    }
  }

  const queryEntries: Array<[string, WidgetQueryRequestDefinition]> = []
  for (const [name, query] of Object.entries(value.queries)) {
    if (!isWidgetQuery(query)) {
      return {
        ok: false,
        message: `queries.${name} の形式が正しくありません。`,
      }
    }
    queryEntries.push([name, query])
  }

  let formulas: Record<string, string> | undefined
  if (value.formulas !== undefined) {
    if (!isRecord(value.formulas)) {
      return { ok: false, message: 'formulas は文字列のマップにしてください。' }
    }
    const formulaEntries: Array<[string, string]> = []
    for (const [name, formula] of Object.entries(value.formulas)) {
      if (typeof formula !== 'string') {
        return {
          ok: false,
          message: `formulas.${name} は文字列にしてください。`,
        }
      }
      formulaEntries.push([name, formula])
    }
    formulas = Object.fromEntries(formulaEntries)
  }

  const time: WidgetTime = {
    as_of: value.time.as_of,
    ...(value.time.range === undefined ? {} : { range: value.time.range }),
  }
  return {
    ok: true,
    request: {
      queries: Object.fromEntries(queryEntries),
      ...(formulas === undefined ? {} : { formulas }),
      viz: value.viz,
      time,
    },
    viz: value.viz,
    asOf: value.time.as_of,
  }
}
