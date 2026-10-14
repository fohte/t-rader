const PHASE_LABEL: Record<string, string> = {
  pending: 'PENDING',
  running: 'RUNNING',
  completed: 'COMPLETED',
  failed: 'FAILED',
}

export function taskPhaseLabel(phase: string): string {
  return PHASE_LABEL[phase] ?? phase
}
