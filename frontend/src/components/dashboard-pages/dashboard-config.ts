import { Result } from 'neverthrow'
import { parseDocument } from 'yaml'

import type { components } from '#lib/api/schema.gen'
import {
  isSupportedWidgetVisualization,
  parseWidgetQueryRequest,
  type SupportedWidgetVisualization,
} from '#lib/widget-definition'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
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
    const parsedDefinition = parseWidgetQueryRequest(definition)
    if (!parsedDefinition.ok) {
      issues.push({
        path:
          parsedDefinition.path == null
            ? path
            : `${path}.${parsedDefinition.path}`,
        message: parsedDefinition.message,
      })
      return
    }
    if (!isSupportedWidgetVisualization(parsedDefinition.viz)) {
      issues.push({
        path: `${path}.viz`,
        message:
          'viz.type は treemap、toplist または table を指定してください。',
      })
      return
    }

    if (
      typeof widgetTitle === 'string' &&
      (width === 'half' || width === 'full')
    ) {
      widgets.push({
        title: widgetTitle,
        width,
        definition: {
          ...parsedDefinition.request,
          viz: parsedDefinition.viz,
        },
      })
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
