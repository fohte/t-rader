export function positiveAllocationValue(
  value: number | null | undefined,
): number {
  return Math.max(value ?? 0, 0)
}

export function allocationTotal(values: (number | null | undefined)[]): number {
  return values.reduce<number>(
    (sum, value) => sum + positiveAllocationValue(value),
    0,
  )
}
