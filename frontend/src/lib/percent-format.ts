type PercentFractionDigits = 1 | 2

type PercentFormatOptions = {
  fractionDigits?: PercentFractionDigits
  signed?: boolean
  negativeSign?: '-' | '−'
  omitSignWhenRoundedToZero?: boolean
}

const percentFormatters = {
  1: new Intl.NumberFormat('ja-JP', {
    style: 'percent',
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  }),
  2: new Intl.NumberFormat('ja-JP', {
    style: 'percent',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }),
} satisfies Record<PercentFractionDigits, Intl.NumberFormat>

function isRoundedToZero(value: number, fractionDigits: PercentFractionDigits) {
  return percentFormatters[fractionDigits]
    .formatToParts(Math.abs(value))
    .filter(({ type }) => type === 'integer' || type === 'fraction')
    .every(({ value: part }) => /^0+$/.test(part))
}

export function formatPercentValue(
  value: number,
  {
    fractionDigits = 1,
    signed = false,
    negativeSign = '−',
    omitSignWhenRoundedToZero = false,
  }: PercentFormatOptions = {},
): string {
  const formatted = percentFormatters[fractionDigits].format(Math.abs(value))
  if (
    !signed ||
    value === 0 ||
    (omitSignWhenRoundedToZero && isRoundedToZero(value, fractionDigits))
  ) {
    return formatted
  }

  return `${value > 0 ? '+' : negativeSign}${formatted}`
}
