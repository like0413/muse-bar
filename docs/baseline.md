# Development baseline

Recorded on 2026-08-18 before Muse Bar business code was added.

## Environment

- Operating system: Windows
- Node.js: 24.19.0, managed by Vite+
- Package manager: pnpm 11.22.0, managed by Vite+
- Vite+ environment: `vp env doctor` passes all checks

## Baseline commands

| Command                                            | Result | Notes                                                                      |
| -------------------------------------------------- | ------ | -------------------------------------------------------------------------- |
| `vp install`                                       | Pass   | Dependencies were already up to date.                                      |
| `vp run type-check`                                | Pass   | Vue TypeScript project compiles without errors.                            |
| `vp build`                                         | Pass   | The original Vue template produces a frontend bundle.                      |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Pass   | The original Tauri Rust crate compiles.                                    |
| `vp check`                                         | Fail   | Generated `auto-imports.d.ts` needs formatting. This is fixed in step 0.2. |
| `vp test`                                          | Fail   | The template has no test files. Test entry points are added in step 0.2.   |

Directly running `pnpm run type-check` could not read Corepack's cached pnpm
directory in the restricted development environment. Project commands therefore
use the Vite+ entry points (`vp` and `vp run`) documented in `AGENTS.md`.

## Baseline conclusion

The unmodified application compiles in both the Vue and Rust layers. The two
failing checks are development-harness gaps rather than product-code defects:
formatting of a generated declaration file and the absence of test files.
