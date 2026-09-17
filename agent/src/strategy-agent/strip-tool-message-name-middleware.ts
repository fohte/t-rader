import type { BaseMessage } from '@langchain/core/messages'
import { ToolMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

// ToolNode は tool 実行結果の ToolMessage に必ず name を設定するが、OpenAI の
// tool ロールにその項目はなく、一部の上流はこれを拒否する。
const stripName = (message: BaseMessage): BaseMessage => {
  if (!ToolMessage.isInstance(message) || message.name === undefined) {
    return message
  }
  // artifact/metadata はモデルに送られないフィールドのため引き継がない。
  return new ToolMessage({
    content: message.content,
    tool_call_id: message.tool_call_id,
    additional_kwargs: message.additional_kwargs,
    response_metadata: message.response_metadata,
    ...(message.id !== undefined ? { id: message.id } : {}),
    ...(message.status !== undefined ? { status: message.status } : {}),
  })
}

export const stripToolMessageNameMiddleware = createMiddleware({
  name: 'stripToolMessageNameMiddleware',
  wrapModelCall: (request, handler) =>
    handler({ ...request, messages: request.messages.map(stripName) }),
})
