import type { BaseMessage } from '@langchain/core/messages'
import { createMiddleware } from 'langchain'

// ToolNode は tool 実行結果の ToolMessage に、AgentNode は中間ターンの
// AIMessage (2 回目以降のモデル呼び出しに含まれる、直前のモデル応答) に、
// それぞれ無条件に name を設定する (langchain の AgentNode.js 実装による)。
// OpenAI の chat completions schema はロールを区別せず message.name を素通り
// させるが、一部の上流 (opencode-go の glm-5.3-flash 等) は tool/assistant
// いずれの name も拒否するため、送信前に落とす。
// サブクラスのコンストラクタで組み直さず prototype を維持したクローンにしている
// のは、langchain 1.5 の MessageStructure 型パラメータをここで特殊化しない限り
// AIMessage の tool_calls 等の型が never に潰れて再構築できないため。
const stripName = (message: BaseMessage): BaseMessage => {
  if (message.name === undefined) {
    return message
  }
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
