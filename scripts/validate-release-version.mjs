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

if (
  stableVersionPattern.test(latestVersion ?? '') &&
  compareVersions(nextVersion, latestVersion) <= 0
) {
  throw new Error(`Release version ${nextVersion} must be newer than ${latestVersion}`)
}
