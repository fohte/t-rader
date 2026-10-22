import type { Result } from 'neverthrow'
import { err, errAsync, ok, okAsync } from 'neverthrow'

import type { AgentConfig } from '#strategy-agent/agent-config-client'
import { isPlainObject } from '#strategy-agent/agent-graph/json'
import type { ExecutionToolRequest } from '#strategy-agent/execution-tool-client'
import type {
  FetchSubagentDefinition,
  SubagentDefinition,
} from '#strategy-agent/subagent-definition-client'
import { parseSubagentDefinition } from '#strategy-agent/subagent-definition-client'

export interface TaskExecutionSnapshot {
  readonly agentConfig: AgentConfig
  readonly subagentDefinitions: ReadonlyMap<string, SubagentDefinition>
}

interface SnapshotMcpClient {
  callExecutionTool?: (
    request: ExecutionToolRequest,
  ) => Promise<Result<unknown, Error>>
  close(): Promise<void>
}

export type CreateSnapshotMcpClient = (
  strategyId: string,
  executionId: string,
  toolModels: Readonly<Record<string, string>>,
) => SnapshotMcpClient

export type FetchTaskExecutionSnapshot = (
  strategyId: string,
  taskId: string,
) => Promise<Result<TaskExecutionSnapshot | undefined, Error>>

const isRecordOfStrings = (value: unknown): value is Record<string, string> =>
  isPlainObject(value) &&
  Object.values(value).every((item) => typeof item === 'string')

const parseTaskStartDefinition = (
  value: unknown,
): Result<TaskExecutionSnapshot, Error> => {
  if (!isPlainObject(value)) {
    return err(new Error('task execution snapshot definition is malformed'))
  }
  const agentConfig = value['agent_config']
  const subagentDefinitions = value['subagent_definitions']
  if (
    !isPlainObject(agentConfig) ||
    typeof agentConfig['agents_md'] !== 'string' ||
    typeof agentConfig['agent_graph'] !== 'string' ||
    !isRecordOfStrings(agentConfig['skills']) ||
    !Array.isArray(subagentDefinitions)
  ) {
    return err(new Error('task execution snapshot definition is incomplete'))
  }

  const definitions = new Map<string, SubagentDefinition>()
  for (const rawDefinition of subagentDefinitions) {
    const parsed = parseSubagentDefinition(rawDefinition)
    if (parsed.isErr()) return err(parsed.error)
    if (definitions.has(parsed.value.key)) {
      return err(
        new Error('task execution snapshot contains duplicate subagent keys'),
      )
    }
    definitions.set(parsed.value.key, parsed.value)
  }

  return ok({
    agentConfig: {
      agentsMd: agentConfig['agents_md'],
      skills: agentConfig['skills'],
      agentGraph: agentConfig['agent_graph'],
    },
    subagentDefinitions: definitions,
  })
}

const parseTaskExecutionSnapshot = (
  value: unknown,
): Result<TaskExecutionSnapshot | undefined, Error> => {
  if (!isPlainObject(value) || !('definition' in value)) {
    return err(new Error('task execution snapshot response is malformed'))
  }
  if (value['definition'] === null) return ok(undefined)
  if (typeof value['digest'] !== 'string') {
    return err(new Error('task execution snapshot digest is missing'))
  }
  return parseTaskStartDefinition(value['definition'])
}

const toError = (error: unknown): Error =>
  error instanceof Error
    ? error
    : new Error('task execution snapshot request failed', { cause: error })

export const createTaskExecutionSnapshotFetcher =
  (createMcpClient: CreateSnapshotMcpClient): FetchTaskExecutionSnapshot =>
  async (strategyId, taskId) =>
    Promise.resolve()
      .then(() => createMcpClient(strategyId, taskId, {}))
      .then((client) => {
        const callExecutionTool = client.callExecutionTool
        const result: Promise<Result<unknown, Error>> =
          callExecutionTool === undefined
            ? Promise.reject(
                new Error('execution MCP tool dispatcher is not configured'),
              )
            : Promise.resolve()
                .then(() =>
                  callExecutionTool({
                    name: 'read_task_execution_snapshot',
                    arguments: {},
                  }),
                )
                .catch((error: unknown) => err(toError(error)))
        return result
          .then((response) => response.andThen(parseTaskExecutionSnapshot))
          .catch((error: unknown) => err(toError(error)))
          .finally(() => client.close().catch(() => undefined))
      })
      .catch((error: unknown) => err(toError(error)))

export const createSnapshotSubagentDefinitionFetcher = (
  snapshot: TaskExecutionSnapshot | undefined,
  fallback: FetchSubagentDefinition,
): FetchSubagentDefinition => {
  if (snapshot === undefined) return fallback
  return (key) => {
    const definition = snapshot.subagentDefinitions.get(key)
    return definition === undefined
      ? errAsync(
          new Error(`subagent definition ${key} is missing from task snapshot`),
        )
      : okAsync(definition)
  }
}
