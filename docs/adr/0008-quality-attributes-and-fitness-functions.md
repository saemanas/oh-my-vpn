# ADR-0008: Quality Attributes and Fitness Functions

## Status

Accepted

## Datetime

2026-03-04T03:18:00+07:00

## Context

Oh My VPN has well-defined non-functional requirements in the PRD (NFR-PERF, NFR-SEC, NFR-REL, NFR-INT), but no mechanism to continuously verify these properties as the codebase evolves. A solo developer maintaining 3 cloud providers across a Rust backend and TypeScript frontend needs automated guardrails to prevent silent quality erosion.

Without fitness functions, NFR compliance is verified only through manual testing -- which decays over time, especially under solo development pressure.

## Decision Drivers

- **Core value protection**: Security is the product's reason to exist -- a credential leak or DNS leak invalidates the entire value proposition
- **Cost risk**: Orphaned servers from reliability failures directly cost users money with no ops team to intervene
- **Solo developer constraint**: No code review partner means automated checks must catch what a reviewer would
- **Incremental adoption**: Cannot invest in all quality dimensions simultaneously -- need a phased approach

## Considered Options

1. **No formal fitness functions** -- rely on manual testing and code review
2. **All 8 catalog attributes from day one** -- comprehensive but unsustainable for a solo developer
3. **Top 3 attributes for MVP, expand to 5 in v1.1** -- phased adoption matching release cadence

## Decision Outcome

Chosen option: "Top 3 attributes for MVP, expand to 5 in v1.1", because it balances protection of core value (security, reliability, performance) with solo developer capacity. The phased approach avoids CI pipeline bloat before the codebase is mature enough to benefit from maintainability and testability metrics.

### Consequences

- **Good**: Critical quality attributes (security, reliability) are guarded from the first CI pipeline
- **Good**: Phased approach prevents fitness function fatigue -- each attribute is added when the codebase has enough code to measure meaningfully
- **Good**: Cost-gated weekly tests (P-3, P-4) prevent runaway cloud API costs from E2E performance tests
- **Bad**: Maintainability and testability are unguarded during MVP development -- technical debt may accumulate before v1.1 fitness functions activate
- **Neutral**: Quarterly review cadence adds a recurring time commitment but ensures thresholds remain relevant

### Selected Attributes and Phase

| Priority | Attribute | Phase | Rationale |
| --- | --- | --- | --- |
| 1 | Security | MVP | Core value -- privacy by destruction, zero-log, ephemeral keys |
| 2 | Reliability | MVP | Orphaned servers = billing leaks + security exposure |
| 3 | Performance | MVP | Provisioning speed is the primary UX differentiator |
| 4 | Maintainability | v1.1 | Solo developer + 3 providers requires modular boundaries |
| 5 | Testability | v1.1 | Fitness functions depend on automated tests |

## Links

- Defines: [quality-attributes.md](../architecture/quality-attributes.md)
- Principles: Fail Fast (SYSTEM.md §3.B #5), Documentation as Code (SYSTEM.md §3.B #6)
