// backend の validate_skill_name と同じ ^[a-z0-9][a-z0-9_-]*$ を検証する
const SKILL_NAME_RE = /^[a-z0-9][a-z0-9_-]*$/

export const SKILL_NAME_ERROR_EMPTY = 'skill 名を入力してください'
export const SKILL_NAME_ERROR_INVALID =
  'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます'

export function validateSkillName(name: string): string | null {
  if (name === '') return SKILL_NAME_ERROR_EMPTY
  if (!SKILL_NAME_RE.test(name)) return SKILL_NAME_ERROR_INVALID
  return null
}
