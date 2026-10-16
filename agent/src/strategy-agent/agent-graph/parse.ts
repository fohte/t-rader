import { err, ok, Result } from 'neverthrow'
import { parse as parseYamlString } from 'yaml'
import { z } from 'zod'

import type {
  AgentGraphConfig,
  AgentGraphPhase,
} from '#strategy-agent/agent-graph/types'

const timeoutMinutesSchema = z.number().int().min(1).max(360)

export class AgentGraphParseError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'AgentGraphParseError'
  }
}

const agentGraphPhaseSchema = z
  .object({
    key: z.string(),
    label: z.string(),
    model: z.string(),
    reasoning_effort: z.string().optional(),
    prompt: z.string(),
    for_each: z.string().optional(),
    label_field: z.string().optional(),
    max_parallel: z.number().optional(),
    timeout_minutes: timeoutMinutesSchema.optional(),
    skills: z.array(z.string()).default([]),
    subagents: z.array(z.string()).default([]),
    max_subagent_runs: z.number().int().min(0).max(4_294_967_295).optional(),
    // 省略 (undefined) と明示的な空配列を区別するため .default([]) は付けない。
    tools: z.array(z.string()).optional(),
    output: z.record(z.string(), z.unknown()).default({}),
  })
  .superRefine((phase, context) => {
    if (phase.subagents.length > 0 && phase.max_subagent_runs === undefined) {
      context.addIssue({
        code: 'custom',
        message: 'max_subagent_runs is required when subagents are configured',
        path: ['max_subagent_runs'],
      })
    }
  })

const agentGraphConfigSchema = z.object({
  timeout_minutes: timeoutMinutesSchema.optional(),
  tool_limits: z
    .record(z.string(), z.number().int().min(0).max(4_294_967_295))
    .optional(),
  tool_models: z.record(z.string(), z.string()).optional(),
  phases: z.array(agentGraphPhaseSchema),
})

const toAgentGraphPhase = (
  raw: z.infer<typeof agentGraphPhaseSchema>,
): AgentGraphPhase => ({
  key: raw.key,
  label: raw.label,
  model: raw.model,
  ...(raw.reasoning_effort !== undefined
    ? { reasoningEffort: raw.reasoning_effort }
    : {}),
  prompt: raw.prompt,
  ...(raw.for_each !== undefined ? { forEach: raw.for_each } : {}),
  ...(raw.label_field !== undefined ? { labelField: raw.label_field } : {}),
  ...(raw.max_parallel !== undefined ? { maxParallel: raw.max_parallel } : {}),
  ...(raw.timeout_minutes !== undefined
    ? { timeoutMinutes: raw.timeout_minutes }
    : {}),
  skills: raw.skills,
  subagents: raw.subagents,
  ...(raw.max_subagent_runs !== undefined
    ? { maxSubagentRuns: raw.max_subagent_runs }
    : {}),
  ...(raw.tools !== undefined ? { tools: raw.tools } : {}),
  output: raw.output,
})

const parseYaml = Result.fromThrowable(
  (yamlText: string): unknown => parseYamlString(yamlText),
  (error): AgentGraphParseError =>
    new AgentGraphParseError('agent_graph is not valid YAML', error),
)

export const parseAgentGraph = (
  yamlText: string,
): Result<AgentGraphConfig, AgentGraphParseError> => {
  if (yamlText.trim() === '') {
    return err(new AgentGraphParseError('agent_graph is not configured'))
  }

  return parseYaml(yamlText).andThen((parsed) => {
    const validated = agentGraphConfigSchema.safeParse(parsed)
    if (!validated.success) {
      return err(
        new AgentGraphParseError(
          `agent_graph does not match the expected shape: ${validated.error.message}`,
        ),
      )
    }
    return ok({
      ...(validated.data.timeout_minutes !== undefined
        ? { timeoutMinutes: validated.data.timeout_minutes }
        : {}),
      ...(validated.data.tool_limits !== undefined
        ? { toolLimits: validated.data.tool_limits }
        : {}),
      ...(validated.data.tool_models !== undefined
        ? { toolModels: validated.data.tool_models }
        : {}),
      phases: validated.data.phases.map(toAgentGraphPhase),
    })
  })
}
