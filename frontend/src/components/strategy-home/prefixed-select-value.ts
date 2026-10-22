export function encodePrefixedSelectValue(
  value: string | undefined,
  prefix: string,
  emptyValue: string,
): string {
  return value == null || value === '' ? emptyValue : `${prefix}${value}`
}

export function decodePrefixedSelectValue(
  value: string | null | undefined,
  prefix: string,
  emptyValues: readonly string[],
): string | undefined {
  if (value == null || emptyValues.includes(value)) return undefined
  return value.slice(prefix.length)
}
