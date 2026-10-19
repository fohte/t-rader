import type { components } from '#lib/api/schema.gen'

export type WidgetQueryDefinitions =
  components['schemas']['WidgetQueryRequest']['queries']

const numberFormat = new Intl.NumberFormat('ja-JP', {
  maximumFractionDigits: 2,
})

const changeFormat = new Intl.NumberFormat('ja-JP', {
  style: 'percent',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})

function isRoundedToZero(value: number): boolean {
  return changeFormat
    .formatToParts(Math.abs(value))
    .filter(({ type }) => type === 'integer' || type === 'fraction')
    .every(({ value: part }) => /^0+$/.test(part))
}

export function formatWidgetValue(
  name: string,
  value: number,
  queries: WidgetQueryDefinitions,
): string {
  if (queries[name]?.metric !== 'change') return numberFormat.format(value)

  if (isRoundedToZero(value)) return changeFormat.format(0)

  const sign = value < 0 ? '-' : '+'
  return `${sign}${changeFormat.format(Math.abs(value))}`
}
