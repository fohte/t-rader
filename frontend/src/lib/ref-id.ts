export interface GroupRefIdParts {
  axisKey: string
  groupKey: string
}

export function parseGroupRefId(id: string): GroupRefIdParts | null {
  const separatorIndex = id.indexOf('/')
  if (
    separatorIndex <= 0 ||
    separatorIndex === id.length - 1 ||
    id.indexOf('/', separatorIndex + 1) !== -1
  ) {
    return null
  }

  return {
    axisKey: id.slice(0, separatorIndex),
    groupKey: id.slice(separatorIndex + 1),
  }
}
