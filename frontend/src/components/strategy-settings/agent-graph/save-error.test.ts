import { describe, expect, it } from 'vitest'

import { extractPhaseKeyFromSaveError } from '#components/strategy-settings/agent-graph/save-error'

// メッセージは backend/src/services/agent_graph.rs の AgentGraphError (thiserror) の
// #[error(...)] 文言そのまま。実際の Display 出力例を使う。
describe('extractPhaseKeyFromSaveError', () => {
  it.each([
    [
      'agent_graph is not valid YAML: mapping values are not allowed here',
      null,
    ],
    ['phase key "plan" is duplicated', 'plan'],
    [
      'phase "investigate": for_each must be in the form "<phase_key>.<field>", got "plan"',
      'investigate',
    ],
    [
      'phase "investigate": for_each references unknown phase "missing" (must be an earlier phase)',
      'investigate',
    ],
    [
      'phase "investigate": for_each references field "missing_field" which is not defined in phase "plan"\'s output',
      'investigate',
    ],
    [
      'phase "investigate": for_each references field "hypotheses" in phase "plan", which is not an array',
      'investigate',
    ],
  ])('%s -> %s', (message, expected) => {
    expect(extractPhaseKeyFromSaveError(message)).toBe(expected)
  })
})
