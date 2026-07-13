import type { AgentConfig } from '@/strategy-agent/agent-config-client'

// AGENTS.md and skills are both fetched from the backend and folded
// together into one system prompt.
export const buildSystemPrompt = (
  config: Pick<AgentConfig, 'agentsMd' | 'skills'>,
): string => {
  const sections = [config.agentsMd.trim()]
  for (const [name, body] of Object.entries(config.skills)) {
    const trimmedBody = body.trim()
    if (trimmedBody === '') continue
    sections.push(`# Skill: ${name}\n\n${trimmedBody}`)
  }
  return sections.filter((section) => section !== '').join('\n\n')
}
