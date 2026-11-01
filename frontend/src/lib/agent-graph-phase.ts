export function isSavedCodeOperationPhase(
  value: Record<string, unknown>,
): value is Record<string, unknown> & {
  kind: 'operation'
  operation: 'evaluate_saved_code'
  code_id: string
  model?: null
} {
  return (
    value['kind'] === 'operation' &&
    value['operation'] === 'evaluate_saved_code' &&
    typeof value['code_id'] === 'string' &&
    (value['model'] === undefined || value['model'] === null)
  )
}
