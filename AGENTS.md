# Project Context Prompt

## 1. Documentation References

**On any change that affects module boundaries, dependencies, or system structure:**

1. Review the files below for relevance
2. Update diagrams and prose to reflect the change
3. Write a new ADR if the change involves a significant decision

**Phase tags** control which documents agents load per task context:

| Phase | When to load | When to skip |
| --- | --- | --- |
| `ideation` | Brainstorming, product discovery | Planning, implementation |
| `plan/execution` | Planning, implementation | Ideation (already internalized in later docs) |

Default phase (no tag) is `plan/execution`.

### A. Product

<!-- scope: requirements, user stories, prioritization, release phasing -->

- [brainstorming](docs/brainstorming/2026-03-03-0433-brainstorming.md) <!-- phase: ideation -->
- [product-brief](docs/product-brief/2026-03-03-0537-product-brief.md) <!-- phase: ideation -->
- [prd](docs/prd/2026-03-03-0537-prd.md)

### B. UX Design

<!-- scope: user journeys, interaction patterns, component strategy, accessibility -->

- [ux-design](docs/ux-design/2026-03-03-1619-ux-design.md)

### C. UI Design

<!-- scope: design tokens, component specs, wireframes, layout system, theme -->

- [ui-design](docs/ui-design/2026-03-04-0123-ui-design.md)
- [tokens.css](docs/ui-design/tokens.css)

### D. Architecture

<!-- scope: system boundary, module structure, dependencies, deployment, cross-cutting concerns, quality attributes, drift prevention -->

- [context.md](docs/architecture/context.md)
- [containers.md](docs/architecture/containers.md)
- [deployment.md](docs/architecture/deployment.md)
- [cross-cutting-concepts.md](docs/architecture/cross-cutting-concepts.md)
- [quality-attributes.md](docs/architecture/quality-attributes.md)
- [drift-prevention.md](docs/architecture/drift-prevention.md)

### E. ADR

<!-- scope: resolved technical decisions and constraints -->

- [ADR-0001: Use wireguard-go with wg-quick](docs/adr/0001-use-wireguard-go-with-wg-quick.md)
- [ADR-0002: Use Rust SDK for Cloud Providers](docs/adr/0002-use-rust-sdk-for-cloud-providers.md)
- [ADR-0003: No Network Extension for MVP](docs/adr/0003-no-network-extension-for-mvp.md)
- [ADR-0004: Ephemeral SSH Keys Per Session](docs/adr/0004-ephemeral-ssh-keys-per-session.md)
- [ADR-0005: Use Provider Pricing API](docs/adr/0005-use-provider-pricing-api.md)
- [ADR-0006: All Providers in MVP](docs/adr/0006-all-providers-in-mvp.md)
- [ADR-0007: Tauri Updater with GitHub Releases](docs/adr/0007-tauri-updater-with-github-releases.md)
- [ADR-0008: Quality Attributes and Fitness Functions](docs/adr/0008-quality-attributes-and-fitness-functions.md)

### F. Data Model

<!-- scope: entity catalog, ER diagrams, schema definitions, access patterns, migration strategy -->

- [2026-03-04-1712-data-model.md](docs/data-model/2026-03-04-1712-data-model.md)

### G. API Design

<!-- scope: API contracts, endpoint definitions, IPC command schemas, versioning strategy -->
<!-- Format: - [{file_name}]({relative_path}) -->

### H. Milestone

<!-- scope: implementation decomposition, dependency ordering, progress tracking -->
<!-- Format: - [{file_name}]({relative_path}) -->

### I. References

<!-- scope: style guides, external references, supplementary documentation -->
<!-- Format: - [{file_name}]({relative_path}) -->

---

## 2. Project Stack

<!-- No manifest file detected (package.json, Cargo.toml, pyproject.toml, go.mod). -->
<!-- Update this section when the project stack is determined. -->

---
