import { useQueries } from '@tanstack/react-query'

import type { DashboardConfig } from '#components/dashboard-pages/dashboard-config'
import type { DashboardWidgetPageState } from '#components/dashboard-pages/dashboard-page-view'
import { $api } from '#lib/api/client'
import { getApiErrorMessage, getApiErrorStatus } from '#lib/api/error'

function createWidgetQueryBody(
  definition: DashboardConfig['widgets'][number]['definition'],
  asOf: string | null | undefined,
) {
  const { queries, formulas, viz, time } = definition
  const requestTime =
    asOf === undefined
      ? time
      : asOf === null
        ? undefined
        : { ...time, as_of: asOf }
  return {
    queries,
    ...(formulas == null ? {} : { formulas }),
    viz,
    ...(requestTime == null ? {} : { time: requestTime }),
  }
}

export function useDashboardWidgetQueries(
  config: DashboardConfig | null,
  asOf: string | null | undefined,
): DashboardWidgetPageState[] {
  const queries = useQueries({
    queries:
      config?.widgets.map((widget) =>
        $api.queryOptions('post', '/api/widgets/query', {
          body: createWidgetQueryBody(widget.definition, asOf),
        }),
      ) ?? [],
  })

  return (
    config?.widgets.map((widget, index) => {
      const query = queries[index]
      const errorStatus =
        query?.isError === true ? getApiErrorStatus(query.error) : null
      return {
        key: `${String(index)}-${widget.title}`,
        title: widget.title,
        width: widget.width,
        definition: {
          queries: widget.definition.queries,
          viz: widget.definition.viz,
        },
        isPending: query?.isPending ?? true,
        errorMessage:
          query?.isError === true
            ? errorStatus === 404
              ? '基準日以前のデータがありません。基準日を変更してください。'
              : getApiErrorMessage(
                  query.error,
                  'ウィジェットのデータを取得できませんでした。',
                )
            : null,
        result: query?.data ?? null,
      }
    }) ?? []
  )
}
