import type { RunAgentGraphContext } from '#strategy-agent/agent-graph/run-agent-graph/types'
import {
  isPhaseTimeoutSignal,
  phaseTimeoutMessage,
} from '#strategy-agent/phase-timeout'

const DEADLINE_EXCEEDED_MESSAGE = 'deadline を超過したため実行を中断しました'

export const isExecutionAborted = (context: RunAgentGraphContext): boolean =>
  context.deadlineSignal?.aborted === true

export const getExecutionAbortMessage = (
  context: RunAgentGraphContext,
): string => {
  const { phaseTimeout } = context
  return isPhaseTimeoutSignal(context.deadlineSignal, phaseTimeout)
    ? phaseTimeoutMessage(phaseTimeout)
    : DEADLINE_EXCEEDED_MESSAGE
}
