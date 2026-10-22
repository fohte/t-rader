import { describe, expect, it } from 'vitest'

import { extractPhaseKeyFromSaveError } from '#components/strategy-settings/agent-graph/save-error'

// メッセージは backend/crates/core/application/src/agent_config/graph.rs の AgentGraphError (thiserror) の
// #[error(...)] 文言そのまま。実際の Display 出力例を使う。
describe('extractPhaseKeyFromSaveError', () => {
  it.each([
    [
      'agent_graph is not valid YAML: mapping values are not allowed here',
      null,
    ],
    ['phase key "alpha" is duplicated', 'alpha'],
    [
      'phase "beta": for_each must be in the form "<phase_key>.<field>", got "alpha"',
      'beta',
    ],
    [
      'phase "beta": for_each references unknown phase "missing" (must be an earlier phase)',
      'beta',
    ],
    [
      'phase "beta": for_each references field "missing_field" which is not defined in phase "alpha"\'s output',
      'beta',
    ],
    [
      'phase "beta": for_each references field "sample_field" in phase "alpha", which is not an array',
      'beta',
    ],
  ])('%s -> %s', (message, expected) => {
    expect(extractPhaseKeyFromSaveError(message)).toBe(expected)
  })
})
