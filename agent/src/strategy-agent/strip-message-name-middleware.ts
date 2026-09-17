import type { BaseMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

// langchain が ToolMessage/AIMessage に付与する name を、これを拒否する
// 上流プロバイダ向けに送信前に落とす。
const stripName = (message: BaseMessage): BaseMessage => {
  if (message.name === undefined) {
    return message
  }
  // AIMessage をコンストラクタで組み直すと、langchain 1.5 の MessageStructure
  // 型パラメータを特殊化しない限り tool_calls の型が never に潰れるため、
  // prototype を維持したクローンで name だけ外す。
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion, @typescript-eslint/no-unsafe-argument -- Object.create/getPrototypeOf は any を返すが、message 自身の prototype から作るため BaseMessage のフィールド一式を持つことは自明。
  const clone = Object.create(Object.getPrototypeOf(message)) as BaseMessage
  Object.assign(clone, message, { name: undefined })
  return clone
}

export const stripMessageNameMiddleware = createMiddleware({
  name: 'stripMessageNameMiddleware',
  wrapModelCall: (request, handler) =>
    handler({ ...request, messages: request.messages.map(stripName) }),
})
