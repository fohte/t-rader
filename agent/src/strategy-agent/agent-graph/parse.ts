import { err, ok, Result } from 'neverthrow'
import { parse as parseYamlString } from 'yaml'
import { z } from 'zod'

import type {
  AgentGraphConfig,
  AgentGraphPhase,
  InputBinding,
} from '#strategy-agent/agent-graph/types'

const timeoutMinutesSchema = z.number().int().min(1).max(360)

export class AgentGraphParseError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'AgentGraphParseError'
  }
}

const inputBindingSchema = z.union([
  z
    .object({ literal: z.unknown() })
    .strict()
    .refine((binding) => 'literal' in binding),
  z
    .object({
      prior_output: z
        .object({
          phase: z.string(),
          pointer: z.string(),
          cursor: z.number().int().nonnegative().optional(),
          limit: z.number().int().min(1).max(1_000).optional(),
        })
        .strict(),
    })
    .strict(),
  z
    .object({
      dataset: z
        .object({
          provider: z.string().trim().min(1),
          params: z.record(z.string(), z.unknown()).default({}),
        })
        .strict(),
    })
    .strict(),
])

const phaseBaseShape = {
  key: z.string(),
  label: z.string(),
  for_each: z.string().optional(),
  label_field: z.string().optional(),
  max_parallel: z.number().int().min(1).optional(),
  timeout_minutes: timeoutMinutesSchema.optional(),
  reasoning_effort: z.string().optional(),
  skills: z.array(z.string()).optional(),
  subagents: z.array(z.string()).optional(),
  max_subagent_runs: z.number().int().min(0).max(4_294_967_295).optional(),
  // 省略 (undefined) と明示的な空配列を区別するため .default([]) は付けない。
  tools: z.array(z.string()).optional(),
  input: z.record(z.string(), inputBindingSchema).optional(),
  output: z.record(z.string(), z.unknown()).default({}),
}

const llmPhaseSchema = z
  .object({
    ...phaseBaseShape,
    skills: z.array(z.string()).default([]),
    subagents: z.array(z.string()).default([]),
    kind: z.literal('llm').optional(),
    model: z.string(),
    prompt: z.string(),
  })
  .strict()

const operationPhaseSchema = z
  .object({
    ...phaseBaseShape,
    kind: z.literal('operation'),
    model: z.null().optional(),
    operation: z.literal('evaluate_saved_code'),
    code_id: z.string().min(1),
  })
  .strict()

const agentGraphPhaseSchema = z
  .union([llmPhaseSchema, operationPhaseSchema])
  .superRefine((phase, context) => {
    if (
      (phase.subagents?.length ?? 0) > 0 &&
      phase.max_subagent_runs === undefined
    ) {
      context.addIssue({
        code: 'custom',
        message: 'max_subagent_runs is required when subagents are configured',
        path: ['max_subagent_runs'],
      })
    }
    if (phase.kind === 'operation' && (phase.subagents?.length ?? 0) > 0) {
      context.addIssue({
        code: 'custom',
        message: 'subagents are only supported by llm phases',
        path: ['subagents'],
      })
    }
    if (phase.kind === 'operation' && phase.for_each !== undefined) {
      context.addIssue({
        code: 'custom',
        message: 'for_each is only supported by llm phases',
        path: ['for_each'],
      })
    }
    if (phase.kind === 'operation') {
      for (const field of [
        'reasoning_effort',
        'tools',
        'skills',
        'max_subagent_runs',
        'max_parallel',
      ] as const) {
        if (phase[field] !== undefined) {
          context.addIssue({
            code: 'custom',
            message: `${field} is only supported by llm phases`,
            path: [field],
          })
        }
      }
    }
  })

const agentGraphConfigSchema = z
  .object({
    timeout_minutes: timeoutMinutesSchema.optional(),
    tool_limits: z
      .record(z.string(), z.number().int().min(0).max(4_294_967_295))
      .optional(),
    tool_models: z.record(z.string(), z.string()).optional(),
    phases: z.array(agentGraphPhaseSchema),
  })
  .superRefine((config, context) => {
    for (const [phaseIndex, phase] of config.phases.entries()) {
      for (const [field, binding] of Object.entries(phase.input ?? {})) {
        if ('dataset' in binding && phase.kind !== 'operation') {
          context.addIssue({
            code: 'custom',
            message: 'dataset bindings are only supported by operation phases',
            path: ['phases', phaseIndex, 'input', field],
          })
        }
        if (!('prior_output' in binding)) continue
        const sourceIndex = config.phases.findIndex(
          (candidate) => candidate.key === binding.prior_output.phase,
        )
        const source = config.phases[sourceIndex]
        if (
          sourceIndex === -1 ||
          sourceIndex >= phaseIndex ||
          source?.kind !== 'operation'
        ) {
          context.addIssue({
            code: 'custom',
            message: 'prior_output must reference an earlier operation phase',
            path: [
              'phases',
              phaseIndex,
              'input',
              field,
              'prior_output',
              'phase',
            ],
          })
        }
        const pointer = binding.prior_output.pointer
        if (pointer !== '' && !pointer.startsWith('/')) {
          context.addIssue({
            code: 'custom',
            message: 'prior_output pointer must be empty or a JSON Pointer',
            path: [
              'phases',
              phaseIndex,
              'input',
              field,
              'prior_output',
              'pointer',
            ],
          })
        }
      }
    }
  })

const toAgentGraphPhase = (
  raw: z.infer<typeof agentGraphPhaseSchema>,
): AgentGraphPhase => ({
  ...(() => {
    const common = {
      key: raw.key,
      label: raw.label,
      ...(raw.for_each !== undefined ? { forEach: raw.for_each } : {}),
      ...(raw.label_field !== undefined ? { labelField: raw.label_field } : {}),
      ...(raw.max_parallel !== undefined
        ? { maxParallel: raw.max_parallel }
        : {}),
      ...(raw.timeout_minutes !== undefined
        ? { timeoutMinutes: raw.timeout_minutes }
        : {}),
      skills: raw.skills ?? [],
      subagents: raw.subagents ?? [],
      ...(raw.max_subagent_runs !== undefined
        ? { maxSubagentRuns: raw.max_subagent_runs }
        : {}),
      ...(raw.tools !== undefined ? { tools: raw.tools } : {}),
      ...(raw.input !== undefined
        ? {
            input: Object.fromEntries(
              Object.entries(raw.input).map(([key, binding]) => [
                key,
                toInputBinding(binding),
              ]),
            ),
          }
        : {}),
      output: raw.output,
    }
    if (raw.kind === 'operation') {
      return {
        ...common,
        kind: 'operation' as const,
        operation: raw.operation,
        codeId: raw.code_id,
      }
    }
    return {
      ...common,
      ...(raw.kind === 'llm' ? { kind: 'llm' as const } : {}),
      model: raw.model,
      ...(raw.reasoning_effort !== undefined
        ? { reasoningEffort: raw.reasoning_effort }
        : {}),
      prompt: raw.prompt,
    }
  })(),
})

const toInputBinding = (
  raw: z.infer<typeof inputBindingSchema>,
): InputBinding => {
  if ('literal' in raw) return { literal: raw.literal }
  if ('prior_output' in raw) {
    return {
      priorOutput: {
        phase: raw.prior_output.phase,
        pointer: raw.prior_output.pointer,
        ...(raw.prior_output.cursor !== undefined
          ? { cursor: raw.prior_output.cursor }
          : {}),
        ...(raw.prior_output.limit !== undefined
          ? { limit: raw.prior_output.limit }
          : {}),
      },
    }
  }
  return { dataset: raw.dataset }
}

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
