import { ChatOpenAI } from '@langchain/openai'

export type ChatOpenAIFetch = NonNullable<
  NonNullable<ConstructorParameters<typeof ChatOpenAI>[0]>['configuration']
>['fetch']

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
