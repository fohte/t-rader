import type { BaseMessage } from '@langchain/core/messages'
import { ToolMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

// langchain の ToolNode は tool 実行結果の ToolMessage に常に name (tool 名) を
// 設定するが、OpenAI の ChatCompletionToolMessageParam 型には name フィールドが
// 存在しない (content/role/tool_call_id のみ)。@langchain/openai の completions
// 変換はロールを区別せず message.name をそのまま乗せるため、role "tool" にも
// 仕様に無い name が混入する。opencode-go/glm-5.3-flash の上流はこれを
// 400 "name" is not supported by this endpoint で拒否するため、モデルに渡す
// 直前で落とす。
const stripName = (message: BaseMessage): BaseMessage => {
  if (!ToolMessage.isInstance(message) || message.name === undefined) {
    return message
  }
  // artifact/metadata はそもそもモデルに送られないフィールドのため引き継がない
  // (artifact は ToolMessageFields の doc comment より「モデルに送る意図はない」)。
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
