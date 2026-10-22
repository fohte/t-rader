import { WidgetLine } from '#components/widgets/widget-line'
import { WidgetTable } from '#components/widgets/widget-table'
import { WidgetToplist } from '#components/widgets/widget-toplist'
import { WidgetTreemap } from '#components/widgets/widget-treemap'
import type { components } from '#lib/api/schema.gen'
import type { SupportedWidgetVisualization } from '#lib/widget-definition'
import type {
  WidgetDisplayDefinitions,
  WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

export function WidgetVisualizationView({
  queries,
  display,
  result,
  viz,
}: {
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
  result: WidgetQueryResponse
  viz: SupportedWidgetVisualization
}) {
  switch (viz.type) {
    case 'treemap':
      return (
        <WidgetTreemap
          queries={queries}
          display={display}
          result={result}
          viz={viz}
        />
      )
    case 'toplist':
      return (
        <WidgetToplist
          queries={queries}
          display={display}
          result={result}
          viz={viz}
        />
      )
    case 'table':
      return (
        <WidgetTable
          queries={queries}
          display={display}
          result={result}
          viz={viz}
        />
      )
    case 'line':
      return (
        <WidgetLine
          queries={queries}
          display={display}
          result={result}
          viz={viz}
        />
      )
    default:
      return viz satisfies never
  }
}
