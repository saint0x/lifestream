# Workspace Agent Notes

## Fozzy Tooling Distinction

- `fz` is the FozzyLang compiler toolchain.
- `fozzy` is the determinism engine and the primary system-testing surface.
- `fz` may be used as a test runner when that is the best fit, but it should not be treated as a synonym for `fozzy`.
- Treat `fz` vs `fozzy` like `bun` vs `bun test`: related tooling, different primary roles.
- Prefer `fozzy` first for determinism, trace validation, and production-readiness checks; use `fz` when the compiler or its test-runner surface is specifically the right tool.
