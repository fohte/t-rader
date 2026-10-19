import { useMemo } from 'react'

import { MarkdownBodyWidgetView } from '#components/note-detail/markdown-body-widget-view'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'
import {
  isSupportedWidgetVisualization,
  parseWidgetDefinition,
} from '#lib/widget-definition'

type WidgetQueryRequest = components['schemas']['WidgetQueryRequest']

const EMPTY_REQUEST: WidgetQueryRequest = {
  queries: {},
  viz: { type: 'toplist', value: 'placeholder', order: 'desc', limit: 0 },
}

function statusOf(error: unknown): number | undefined {
  if (
    typeof error !== 'object' ||
    error === null ||
    Array.isArray(error) ||
    !('status' in error) ||
    typeof error.status !== 'number'
  ) {
    return undefined
  }
  return error.status
}

function messageOf(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    !Array.isArray(error) &&
    'error' in error &&
    typeof error.error === 'string'
  ) {
    return error.error
  }
  return error instanceof Error
    ? error.message
    : 'データを取得できませんでした。'
}

export function MarkdownBodyWidget({ source }: { source: string }) {
  const parsed = useMemo(() => {
    const definition = parseWidgetDefinition(source)
    if (!definition.ok) return definition
    if (definition.asOf === undefined) {
      return {
        ok: false as const,
        message: 'time.as_of はノート内ウィジェットで必須です。',
      }
    }
    return { ...definition, asOf: definition.asOf }
  }, [source])
  const query = $api.useQuery(
    'post',
    '/api/widgets/query',
    { body: parsed.ok ? parsed.request : EMPTY_REQUEST },
    { enabled: parsed.ok },
  )

  if (!parsed.ok) {
    return (
      <MarkdownBodyWidgetView
        state={{ kind: 'definition-error', message: parsed.message }}
      />
    )
  }
  if (query.isPending) {
    return (
      <MarkdownBodyWidgetView state={{ kind: 'loading', asOf: parsed.asOf }} />
    )
  }
  if (query.isError) {
    const status = statusOf(query.error)
    return (
      <MarkdownBodyWidgetView
        state={{
          kind:
            status === 400 || status === 422
              ? 'definition-error'
              : 'data-error',
          asOf: parsed.asOf,
          message: messageOf(query.error),
        }}
      />
    )
  }
  if (!isSupportedWidgetVisualization(parsed.viz)) {
    return (
      <MarkdownBodyWidgetView
        state={{
          kind: 'definition-error',
          asOf: parsed.asOf,
          message: 'この表示形式はノート内ウィジェットでは表示できません。',
        }}
      />
    )
  }
  return (
    <MarkdownBodyWidgetView
      state={{
        kind: 'ready',
        queries: parsed.request.queries,
        result: query.data,
        viz: parsed.viz,
      }}
    />
  )
}
