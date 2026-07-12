import type { AgentCard } from '@a2a-js/sdk'

export interface BuildAgentCardOptions {
  readonly url: string
}

export const buildAgentCard = (options: BuildAgentCardOptions): AgentCard => ({
  name: 'trader-agent',
  description:
    'Executes a t-rader strategy task (AGENTS.md / skills / model configured per strategy) against the backend MCP strategy tools.',
  protocolVersion: '0.3',
  url: options.url,
  version: '0.1.0',
  capabilities: { pushNotifications: true, streaming: false },
  defaultInputModes: ['text'],
  defaultOutputModes: ['text'],
  skills: [
    {
      id: 'strategy-task',
      name: 'Strategy task execution',
      description:
        'Runs one prompt against a single strategy, scoped to that strategy via the strategy_id message metadata, and returns the result text.',
      tags: ['strategy'],
    },
  ],
})
