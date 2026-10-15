import { ChatOpenAI } from '@langchain/openai'

import type { ChatOpenAIFetch } from '#strategy-agent/reasoning-content-middleware'

export type { ChatOpenAIFetch }

export const createStubChatModel = (
  fetch: ChatOpenAIFetch,
  options: { readonly model: string; readonly streaming?: boolean },
): ChatOpenAI =>
  new ChatOpenAI({
    apiKey: 'test-key',
    model: options.model,
    maxRetries: 0,
    streaming: options.streaming ?? false,
    configuration: { baseURL: 'http://localhost', fetch },
  })
