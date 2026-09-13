import type { BaseCallbackHandler } from '@langchain/core/callbacks/base'
import {
  mergeConfigs,
  Runnable,
  RunnableBinding,
} from '@langchain/core/runnables'
import type { ModelRequest } from 'langchain'

// request.model の型 (AgentLanguageModelLike) は RunnableBinding.bound が要求する
// 具象 Runnable より緩いため、実行時に確認できない場合は素通しする。
//
// langchain の _simpleBindTools は RunnableBinding を 1 段しか unwrap しないため、
// 二重ラップを避けて既存の config にマージする。
export const bindModelCallSignal = (
  model: ModelRequest['model'],
  addedConfig: { signal: AbortSignal; callbacks?: BaseCallbackHandler[] },
): ModelRequest['model'] => {
  if (!(model instanceof Runnable)) return model

  return RunnableBinding.isRunnableBinding(model)
    ? new RunnableBinding({
        bound: model.bound,
        config: mergeConfigs(model.config, addedConfig),
        kwargs: model.kwargs ?? {},
      })
    : new RunnableBinding({ bound: model, config: addedConfig, kwargs: {} })
}
