import { Result } from 'neverthrow'
import { parseDocument } from 'yaml'

import type { components } from '#lib/api/schema.gen'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']
type WidgetVisualization = components['schemas']['WidgetVisualizationRequest']

type SupportedWidgetVisualization = Extract<
  WidgetVisualization,
  { type: 'treemap' | 'toplist' }
>

type DashboardWidgetDefinition = Omit<WidgetQueryRequest, 'viz'> & {
  viz: SupportedWidgetVisualization
}

type DashboardWidget = {
  title: string
  width: 'half' | 'full'
  definition: DashboardWidgetDefinition
}

export type DashboardConfig = {
  title: string
  widgets: DashboardWidget[]
}

export type DashboardConfigIssue = {
  path: string
  message: string
}

export type DashboardConfigResult =
  | { ok: true; value: DashboardConfig }
  | { ok: false; issues: DashboardConfigIssue[] }

const supportedMetrics = new Set([
  'change',
  'close',
  'turnover',
  'mkt_cap',
  'per',
  'pbr',
  'indicator',
])
const supportedPeriods = new Set(['1d', '5d', '20d', '60d'])
const supportedDimensions = new Set(['none', 'stock', 'group'])
const supportedAggregations = new Set([
  'mkt_cap_weighted',
  'mean',
  'sum',
  'count',
])

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isOneOf(value: unknown, values: Set<string>): value is string {
  return typeof value === 'string' && values.has(value)
}

function isDate(value: unknown): value is string {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return false
  }

  const parsed = new Date(`${value}T00:00:00.000Z`)
  return (
    !Number.isNaN(parsed.getTime()) &&
    parsed.toISOString().slice(0, 10) === value
  )
}

function isWidgetQuery(value: unknown): boolean {
  if (!isRecord(value)) return false
  if (
    !isOneOf(value['metric'], supportedMetrics) ||
    !isOneOf(value['by'], supportedDimensions)
  ) {
    return false
  }
  if (value['period'] != null && !isOneOf(value['period'], supportedPeriods)) {
    return false
  }
  if (value['agg'] != null && !isOneOf(value['agg'], supportedAggregations)) {
    return false
  }
  if (value['axis'] != null && typeof value['axis'] !== 'string') {
    return false
  }
  if (value['indicator'] != null && typeof value['indicator'] !== 'string') {
    return false
  }
  if ('universe' in value && value['universe'] != null) {
    const universe = value['universe']
    if (!isRecord(universe)) return false
    if (
      typeof universe['group'] === 'string' &&
      Object.keys(universe).length === 1
    ) {
      return true
    }
    return (
      Array.isArray(universe['stocks']) &&
      universe['stocks'].every((stock) => typeof stock === 'string') &&
      Object.keys(universe).length === 1
    )
  }
  return true
}

function isWidgetDefinition(
  value: unknown,
): value is DashboardWidgetDefinition {
  if (!isRecord(value) || !isRecord(value['queries'])) return false
  if (
    Object.keys(value['queries']).length === 0 ||
    !Object.values(value['queries']).every(isWidgetQuery)
  ) {
    return false
  }
  if (
    'formulas' in value &&
    (!isRecord(value['formulas']) ||
      !Object.values(value['formulas']).every(
        (formula) => typeof formula === 'string',
      ))
  ) {
    return false
  }

  const viz = value['viz']
  if (!isRecord(viz)) return false
  const isSupportedViz =
    (viz['type'] === 'treemap' &&
      typeof viz['size'] === 'string' &&
      typeof viz['color'] === 'string') ||
    (viz['type'] === 'toplist' &&
      typeof viz['value'] === 'string' &&
      (viz['order'] === 'asc' || viz['order'] === 'desc') &&
      typeof viz['limit'] === 'number' &&
      Number.isInteger(viz['limit']))
  if (!isSupportedViz) return false

  if ('time' in value) {
    const time = value['time']
    if (!isRecord(time)) return false
    if (time['as_of'] != null && !isDate(time['as_of'])) return false
    if (time['range'] != null && typeof time['range'] !== 'string') return false
  }

  return true
}

function validateDashboard(value: unknown): DashboardConfigResult {
  if (!isRecord(value)) {
    return {
      ok: false,
      issues: [
        {
          path: '$',
          message: 'YAML のトップレベルは mapping にしてください。',
        },
      ],
    }
  }

  const issues: DashboardConfigIssue[] = []
  for (const key of Object.keys(value)) {
    if (key !== 'title' && key !== 'widgets') {
      issues.push({ path: key, message: '未対応の項目です。' })
    }
  }

  const title = value['title']
  if (typeof title !== 'string' || title.trim() === '') {
    issues.push({ path: 'title', message: 'タイトルを指定してください。' })
  }

  const rawWidgets = value['widgets']
  if (!Array.isArray(rawWidgets)) {
    issues.push({
      path: 'widgets',
      message: 'widgets は配列で指定してください。',
    })
    return { ok: false, issues }
  }

  const widgets: DashboardWidget[] = []
  rawWidgets.forEach((rawWidget, index) => {
    const path = `widgets[${String(index)}]`
    if (!isRecord(rawWidget)) {
      issues.push({ path, message: 'ウィジェットは mapping にしてください。' })
      return
    }

    for (const key of Object.keys(rawWidget)) {
      if (
        !['title', 'width', 'queries', 'formulas', 'viz', 'time'].includes(key)
      ) {
        issues.push({ path: `${path}.${key}`, message: '未対応の項目です。' })
      }
    }

    const widgetTitle = rawWidget['title']
    if (typeof widgetTitle !== 'string' || widgetTitle.trim() === '') {
      issues.push({
        path: `${path}.title`,
        message: 'タイトルを指定してください。',
      })
    }

    const width = rawWidget['width']
    if (width !== 'half' && width !== 'full') {
      issues.push({
        path: `${path}.width`,
        message: 'width は half または full を指定してください。',
      })
    }

    const definition: unknown = Object.fromEntries(
      Object.entries(rawWidget).filter(
        ([key]) => key !== 'title' && key !== 'width',
      ),
    )
    if (!isWidgetDefinition(definition)) {
      issues.push({
        path,
        message:
          'queries と treemap または toplist の viz を含むウィジェット定義が必要です。',
      })
      return
    }

    if (
      typeof widgetTitle === 'string' &&
      (width === 'half' || width === 'full')
    ) {
      widgets.push({ title: widgetTitle, width, definition })
    }
  })

  if (issues.length > 0) return { ok: false, issues }
  if (typeof title !== 'string') {
    return {
      ok: false,
      issues: [{ path: 'title', message: 'タイトルを指定してください。' }],
    }
  }
  return { ok: true, value: { title, widgets } }
}

export function parseDashboardConfig(yamlText: string): DashboardConfigResult {
  const document = parseDocument(yamlText)
  if (document.errors.length > 0) {
    return {
      ok: false,
      issues: [{ path: '$', message: 'YAML の構文を確認してください。' }],
    }
  }

  const parsedResult = Result.fromThrowable(
    (doc: typeof document): unknown => {
      const parsed: unknown = doc.toJS()
      return parsed
    },
    () => null,
  )(document)
  if (parsedResult.isErr()) {
    return {
      ok: false,
      issues: [{ path: '$', message: 'YAML の構文を確認してください。' }],
    }
  }

  const parsed: unknown = parsedResult.value
  return validateDashboard(parsed)
}

function decodeBase64Url(hash: string): string | null {
  const encoded = hash.startsWith('#') ? hash.slice(1) : hash
  if (!/^[A-Za-z0-9_-]+={0,2}$/.test(encoded)) return null

  const unpadded = encoded.replace(/=+$/, '')
  if (unpadded.length % 4 === 1) return null
  const paddingLength = encoded.length - unpadded.length
  if (
    paddingLength > 0 &&
    (encoded.length % 4 !== 0 ||
      (paddingLength === 1 && unpadded.length % 4 !== 3) ||
      (paddingLength === 2 && unpadded.length % 4 !== 2))
  ) {
    return null
  }
  const base64 = unpadded
    .replaceAll('-', '+')
    .replaceAll('_', '/')
    .padEnd(unpadded.length + ((4 - (unpadded.length % 4)) % 4), '=')
  const binaryResult = Result.fromThrowable(
    (value: string) => atob(value),
    () => null,
  )(base64)
  if (binaryResult.isErr()) return null

  const bytes = Uint8Array.from(binaryResult.value, (character) =>
    character.charCodeAt(0),
  )
  const textResult = Result.fromThrowable(
    (value: Uint8Array) =>
      new TextDecoder('utf-8', { fatal: true }).decode(value),
    () => null,
  )(bytes)
  return textResult.isOk() ? textResult.value : null
}

export function parseDashboardPreviewHash(hash: string): DashboardConfigResult {
  const yamlText = decodeBase64Url(hash)
  if (yamlText == null) {
    return {
      ok: false,
      issues: [
        {
          path: '$preview',
          message: 'プレビュー URL の YAML データを読み取れません。',
        },
      ],
    }
  }
  return parseDashboardConfig(yamlText)
}

export function getInitialDashboardDate(
  config: DashboardConfig,
  fallbackDate: string,
): string {
  return (
    config.widgets.find((widget) => isDate(widget.definition.time?.as_of))
      ?.definition.time?.as_of ?? fallbackDate
  )
}

export function getDashboardApiErrorMessage(
  error: unknown,
  fallback: string,
): string {
  if (error != null && typeof error === 'object') {
    if ('error' in error && typeof error.error === 'string') return error.error
    if (
      'data' in error &&
      error.data != null &&
      typeof error.data === 'object' &&
      'error' in error.data &&
      typeof error.data.error === 'string'
    ) {
      return error.data.error
    }
  }
  return error instanceof Error && error.message !== ''
    ? error.message
    : fallback
}

export function getDashboardApiErrorStatus(error: unknown): number | null {
  if (error == null || typeof error !== 'object' || !('status' in error)) {
    return null
  }
  return typeof error.status === 'number' ? error.status : null
}
