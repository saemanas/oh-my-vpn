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

- [2026-03-04-1726-api-design.md](docs/api-design/2026-03-04-1726-api-design.md)

### H. Milestone

<!-- scope: implementation decomposition, dependency ordering, progress tracking -->

- [2026-03-04-1726-milestone.md](docs/milestone/2026-03-04-1726-milestone.md)

### I. References

<!-- scope: style guides, external references, supplementary documentation -->
<!-- Format: - [{file_name}]({relative_path}) -->

---

## 2. Verification Strategy

Every milestone module's acceptance criteria should include automated verification where feasible.

| Layer | Tool | Scope | When to run |
| --- | --- | --- | --- |
| Unit / Integration | `cargo test` | Rust backend modules | Every module completion |
| E2E (UI) | `tauri-webdriver` skill | Webview UI flows | After UI modules (M5, M6) |
| Lint / Type check | `cargo clippy`, `bun run check` | All source code | Every module completion |

**Constraints:**

- `tauri-webdriver` operates on **debug builds only** -- WebDriver server is excluded from release
- Webview content only -- native system tray and OS menus cannot be automated via WebDriver
- Rust compilation required before E2E runs -- factor build time into estimates

---

## 3. Project Stack

### A. Runtime

| Layer | Technology | Version |
| --- | --- | --- |
| Framework | Tauri | 2 |
| Backend | Rust (edition 2021) | 1.x |
| Frontend | React + TypeScript | 19 / 5.8 |
| Bundler | Vite | 7 |
| Async runtime | Tokio | 1 |

### B. Backend Crates (Cargo.toml)

#### a. Installed

| Crate | Version | Purpose |
| --- | --- | --- |
| `tauri` | 2 | App framework (tray-icon feature) |
| `tauri-plugin-opener` | 2 | URL/file opener plugin |
| `serde` | 1 (derive) | Serialization |
| `serde_json` | 1 | JSON parsing |
| `security-framework` | 3.7 | macOS Keychain access |
| `core-foundation` | 0.10 | macOS framework bindings |

#### b. Planned (verified compatible)

| Crate | Version | Milestone | Purpose |
| --- | --- | --- | --- |
| `tokio` | 1.50 | M2+ | Async runtime |
| `hcloud` | 0.25 | M2.1 | Hetzner Cloud SDK |
| `aws-sdk-ec2` | 1.215 | M2.2 | AWS EC2 SDK |
| `google-cloud-compute-v1` | 2.2 | M2.3 | GCP Compute SDK |
| `x25519-dalek` | 2.0 | M3.1 | WireGuard key exchange |
| `ed25519-dalek` | 2.2 | M4.1 | SSH key generation |
| `zeroize` | 1.8 | M3.1 | Secure memory zeroing |
| `ssh-key` | 0.6 | M4.1 | SSH key parsing (0.x -- evaluate at M4.1) |

All planned crates were tested together via `cargo check` on 2026-03-04 -- no version conflicts.

### C. Frontend Packages (package.json)

| Package | Version | Purpose |
| --- | --- | --- |
| `react` / `react-dom` | ^19.1 | UI library |
| `@tauri-apps/api` | ^2 | Tauri IPC bridge |
| `@tauri-apps/plugin-opener` | ^2 | Opener plugin JS bindings |
| `@vitejs/plugin-react` | ^4.6 | Vite React plugin (v5 major available -- evaluate at M5) |
| `typescript` | ~5.8 | Type checking (5.9 available -- tilde blocks) |
| `vite` | ^7.0 | Build tooling |
| `@tauri-apps/cli` | ^2 | Tauri CLI |

---
