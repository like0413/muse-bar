#!/usr/bin/env node
import { execSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'

const args = process.argv.slice(2)
const push = args.includes('--push')
const level = args.find((arg) => !arg.startsWith('--')) ?? 'patch'
if (!['major', 'minor', 'patch'].includes(level)) {
  console.error(`无效的版本级别: ${level}，可用: major | minor | patch`)
  process.exit(1)
}

const pkgPath = 'package.json'
const confPath = 'src-tauri/tauri.conf.json'
const cargoPath = 'src-tauri/Cargo.toml'
const lockPath = 'src-tauri/Cargo.lock'
const trackedFiles = [pkgPath, confPath, cargoPath, lockPath]

const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'))
const conf = JSON.parse(readFileSync(confPath, 'utf8'))
const cargo = readFileSync(cargoPath, 'utf8')
const cargoVersion = cargo.match(/^version = "(.+)"$/m)?.[1]

// 发布前先确认三处版本一致，避免误 bump
if (pkg.version !== conf.version || pkg.version !== cargoVersion) {
  console.error(
    `三处版本号不一致，请先手动对齐: package.json=${pkg.version} tauri.conf.json=${conf.version} Cargo.toml=${cargoVersion}`,
  )
  process.exit(1)
}

const [major, minor, patch] = pkg.version.split('.').map(Number)
const next = {
  major: `${major + 1}.0.0`,
  minor: `${major}.${minor + 1}.0`,
  patch: `${major}.${minor}.${patch + 1}`,
}[level]

// 收集本地与远程已存在的 v* 标签（远程不可达时忽略），取最高版本
function getHighestTag() {
  const versions = []
  const collect = (output) => {
    for (const match of output.matchAll(/v(\d+)\.(\d+)\.(\d+)/g)) {
      versions.push(match.slice(1).map(Number))
    }
  }
  collect(execSync('git tag --list "v*"', { encoding: 'utf8' }))
  try {
    collect(
      execSync('git ls-remote --tags --refs origin "v*"', { encoding: 'utf8', timeout: 15000 }),
    )
  } catch {
    // 无网络或未配置远程时忽略
  }
  return versions.sort((a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2]).at(-1)
}

// 防降级：目标版本必须严格高于已存在的最高标签
const atMost = (a, b) =>
  a[0] < b[0] || (a[0] === b[0] && a[1] < b[1]) || (a[0] === b[0] && a[1] === b[1] && a[2] <= b[2])
const highest = getHighestTag()
if (highest && atMost(next.split('.').map(Number), highest)) {
  console.error(`目标版本 v${next} 不高于已存在的最高标签 v${highest.join('.')}，已阻止降级发布。`)
  process.exit(1)
}

pkg.version = next
writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`)
conf.version = next
writeFileSync(confPath, `${JSON.stringify(conf, null, 2)}\n`)
writeFileSync(cargoPath, cargo.replace(/^version = ".*"$/m, `version = "${next}"`))

// 刷新 Cargo.lock，让 CI 里 clippy --locked 等校验通过
execSync('cargo metadata --no-deps --manifest-path src-tauri/Cargo.toml', { stdio: 'inherit' })

execSync(`git add ${trackedFiles.join(' ')}`, { stdio: 'inherit' })
execSync(`git commit -m "chore: 发布 v${next}"`, { stdio: 'inherit' })
execSync(`git tag -a v${next} -m "Release v${next}"`, { stdio: 'inherit' })

if (push) {
  execSync('git push origin HEAD', { stdio: 'inherit' })
  execSync(`git push origin v${next}`, { stdio: 'inherit' })
  console.log(`\n已推送 v${next}，release.yml 已在 GitHub 上触发构建发布。`)
} else {
  console.log(`\n已提交并创建标签 v${next}。推送以触发发布:`)
  console.log(`  git push origin v${next}`)
}
