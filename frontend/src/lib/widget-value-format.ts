import type { components } from '#lib/api/schema.gen'
import { formatPercentValue } from '#lib/percent-format'

export type WidgetQueryDefinitions =
  components['schemas']['WidgetQueryRequest']['queries']

const numberFormat = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

export function formatWidgetValue(
  name: string,
  value: number,
  queries: WidgetQueryDefinitions,
): string {
  if (queries[name]?.metric !== 'change') return numberFormat.format(value)

  return formatPercentValue(value, {
    fractionDigits: 2,
    signed: true,
    negativeSign: '-',
    omitSignWhenRoundedToZero: true,
  })
}
