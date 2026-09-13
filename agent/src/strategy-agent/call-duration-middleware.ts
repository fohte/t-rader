import type { Runnable } from '@langchain/core/runnables'
import { RunnableBinding } from '@langchain/core/runnables'
import { createMiddleware } from 'langchain'

// ChatOpenAI の `timeout` はヘッダ受信までしか縛らず、streaming: true では
// ボディ読み取り中のタイムアウト判定を openai SDK が明示的にスキップする
// (ストリームが流れ続ける限り打ち切られない)。呼び出しごとに新しい
// AbortSignal を作ってモデルに渡すことで、ストリーミング中でも実際の HTTP
// リクエストを呼び出し単位で打ち切る。
//
// signal は modelSettings ではなく RunnableBinding.config 経由で渡す必要がある。
// ChatOpenAI.bindTools/.withConfig は RunnableBinding を作らず自身のクローンに
// defaultOptions として保持するだけなので、AgentNode が invoke 時に渡す
// config (signal 含む) と `{...defaultOptions, ...options}` という単純な
// object spread でマージされ、後勝ちで signal が上書き消失する。
// request.model を RunnableBinding でラップしておけば、AgentNode 側の
// bindTools 処理がそのバインディングの config/kwargs を引き継いだ新しい
// RunnableBinding を返し、invoke 時は @langchain/core の mergeConfigs が
// signal を AbortSignal.any で正しく合成する。
export const createCallDurationMiddleware = (timeoutMs: number) =>
  createMiddleware({
    name: 'callDurationMiddleware',
    wrapModelCall: (request, handler) =>
      handler({
        ...request,
        model: new RunnableBinding({
          // request.model の型 (AgentLanguageModelLike) は invoke/stream 等
          // 最小限の RunnableInterface までしか保証しないが、実行時は
          // createChatModel が返す具象 Runnable (ChatOpenAI) が渡ってくる。
          // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- 上記の通り実行時は必ず具象 Runnable であるため、RunnableBinding.bound に渡すための narrowing。
          bound: request.model as Runnable,
          config: { signal: AbortSignal.timeout(timeoutMs) },
          kwargs: {},
        }),
      }),
  })
