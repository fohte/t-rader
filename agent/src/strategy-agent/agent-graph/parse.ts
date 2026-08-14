import { err, ok, Result } from 'neverthrow'
import { parse as parseYamlString } from 'yaml'
import { z } from 'zod'

import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'

export class AgentGraphParseError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'AgentGraphParseError'
  }
}

// undefined は agent_graph が未設定 (空文字列) であることを表す。backend の
// parse_agent_graph がその場合に Ok(None) を返すのと対応させている。
export type ParsedAgentGraph = AgentGraphConfig | undefined

const agentGraphPhaseSchema = z.object({
  key: z.string(),
  label: z.string(),
  model: z.string(),
  prompt: z.string(),
  runs: z.string().optional(),
  for_each: z.string().optional(),
  label_field: z.string().optional(),
  max_parallel: z.number().optional(),
  skills: z.array(z.string()).default([]),
  tools: z.array(z.string()).default([]),
  output: z.record(z.string(), z.unknown()).default({}),
})

const agentGraphConfigSchema = z.object({
  phases: z.array(agentGraphPhaseSchema),
})

const toAgentGraphPhase = (
  raw: z.infer<typeof agentGraphPhaseSchema>,
): AgentGraphPhase => ({
  key: raw.key,
  label: raw.label,
  model: raw.model,
  prompt: raw.prompt,
  ...(raw.for_each !== undefined ? { forEach: raw.for_each } : {}),
  ...(raw.max_parallel !== undefined ? { maxParallel: raw.max_parallel } : {}),
  skills: raw.skills,
  tools: raw.tools,
  output: raw.output,
})

const parseYaml = Result.fromThrowable(
  (yamlText: string): unknown => parseYamlString(yamlText),
  (error): AgentGraphParseError =>
    new AgentGraphParseError('agent_graph is not valid YAML', error),
)

export const parseAgentGraph = (
  yamlText: string,
): Result<ParsedAgentGraph, AgentGraphParseError> => {
  if (yamlText.trim() === '') return ok(undefined)

  return parseYaml(yamlText).andThen((parsed) => {
    const validated = agentGraphConfigSchema.safeParse(parsed)
    if (!validated.success) {
      return err(
        new AgentGraphParseError(
          `agent_graph does not match the expected shape: ${validated.error.message}`,
        ),
      )
    }
    return ok({ phases: validated.data.phases.map(toAgentGraphPhase) })
  })
}
