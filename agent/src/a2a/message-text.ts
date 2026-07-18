import type { Message } from '@a2a-js/sdk'

export const extractMessageText = (message: Message): string =>
  message.parts
    .filter(
      (part): part is { kind: 'text'; text: string } => part.kind === 'text',
    )
    .map((part) => part.text)
    .join('\n')
