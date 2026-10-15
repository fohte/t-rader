const UUID_PATTERN = /^[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}$/i

export type ExecutionOrigin = {
  strategyId: string
  strategyName: string
  taskId: string
}

export function toExecutionStepId(value: string | null | undefined) {
  return value != null && UUID_PATTERN.test(value) ? value : undefined
}
