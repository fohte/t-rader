import type { components } from '#lib/api/schema.gen'

export type WidgetQueryDefinitions =
  components['schemas']['WidgetQueryRequest']['queries']
export type WidgetDisplayDefinitions = NonNullable<
  components['schemas']['WidgetQueryRequest']['display']
>

const numberFormat = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

const changeFormat = new Intl.NumberFormat('ja-JP', {
  style: 'percent',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  signDisplay: 'never',
})

export function usesWidgetPercentFormat(
  name: string,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions = {},
): boolean {
  const format = display[name]?.format
  return (
    format === 'percent' ||
    (format == null && queries[name]?.metric === 'change')
  )
}

export function formatWidgetValue(
  name: string,
  value: number,
  queries: WidgetQueryDefinitions,
  display: WidgetDisplayDefinitions = {},
): string {
  if (!usesWidgetPercentFormat(name, queries, display)) {
    return numberFormat.format(value)
  }

  const parts = changeFormat.formatToParts(Math.abs(value))
  const formatted = parts.map(({ value: part }) => part).join('')
  const isRoundedToZero = parts
    .filter(({ type }) => type === 'integer' || type === 'fraction')
    .every(({ value: part }) => Number(part) === 0)

  if (isRoundedToZero || Number.isNaN(value)) return formatted

  return `${value > 0 ? '+' : '-'}${formatted}`
}
