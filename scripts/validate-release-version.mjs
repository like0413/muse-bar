import { execFileSync } from 'node:child_process'

const stableVersionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/

const [, , latestVersion, nextVersion] = process.argv

if (!stableVersionPattern.test(nextVersion ?? '')) {
  throw new Error(`Release version must use stable X.Y.Z format, received: ${nextVersion}`)
}

const compareVersions = (left, right) => {
  const leftParts = left.split('.').map(Number)
  const rightParts = right.split('.').map(Number)

  for (let index = 0; index < leftParts.length; index += 1) {
    const difference = leftParts[index] - rightParts[index]
    if (difference !== 0) return difference
  }

  return 0
}

const readStableTags = (args) =>
  execFileSync('git', args, {
    encoding: 'utf8',
    timeout: 15_000,
    windowsHide: true,
  })
    .split(/\r?\n/)
    .map((line) =>
      line
        .trim()
        .split(/\s+/)
        .at(-1)
        ?.replace(/^(?:refs\/tags\/)?v/, ''),
    )
    .filter((version) => stableVersionPattern.test(version ?? ''))

const localTags = readStableTags(['tag', '--list', 'v*'])
const versionFloors = [latestVersion, ...localTags].filter((version) =>
  stableVersionPattern.test(version ?? ''),
)
const highestVersion = versionFloors.sort(compareVersions).at(-1)

if (highestVersion && compareVersions(nextVersion, highestVersion) <= 0) {
  throw new Error(
    `Release version ${nextVersion} must be newer than configuration and fetched Git tags (highest: ${highestVersion})`,
  )
}
