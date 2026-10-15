export interface PhaseTimeout {
  readonly signal: AbortSignal
  readonly phaseKey: string
  readonly timeoutMinutes: number
}

export const isPhaseTimeoutSignal = (
  signal: AbortSignal | undefined,
  phaseTimeout: PhaseTimeout | undefined,
): boolean => {
  // AbortSignal.any は最初に abort した入力の reason を保持する。
  return (
    signal?.aborted === true &&
    phaseTimeout !== undefined &&
    signal.reason === phaseTimeout.signal.reason
  )
}

export const phaseTimeoutMessage = (phaseTimeout: PhaseTimeout): string =>
  `フェーズ ${phaseTimeout.phaseKey} が timeout (${String(phaseTimeout.timeoutMinutes)} 分) を超過`
