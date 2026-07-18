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
        'Runs one prompt against a single t-rader strategy and returns the result text. Callers that know the target strategy can scope the request precisely via the strategy_id message metadata. Callers without that (e.g. a conversational request like "run the long-term strategy on Toyota") can instead just describe the target strategy in the message text; it is resolved against the current strategy list. If the target strategy can\'t be uniquely identified, the task moves to input-required and asks which strategy to use — reply on the same task to continue.',
      tags: ['strategy'],
    },
  ],
})
