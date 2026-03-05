# Drift Prevention

Architecture documentation drifts from reality when changes bypass documentation updates. This document defines the strategies and automation to keep Oh My VPN's architecture docs in sync with code.

---

## 1. Architecture-Significant File Patterns

Changes to these files trigger architecture review:

| Pattern | Reason |
| --- | --- |
| `docs/adr/**` | Decision record added or modified |
| `docs/architecture/**` | Architecture diagram or description changed |
| `src-tauri/Cargo.toml` | New Rust dependency -- potential new module boundary |
| `src-tauri/tauri.conf.json` | Tauri configuration -- affects container/deployment topology |
| `src-tauri/capabilities/**` | Tauri v2 permission system -- affects security boundary |
| `package.json` | New frontend dependency |
| `**/*.schema.*` | Data model change |
| `src-tauri/src/**/*.rs` | Backend module change -- may affect container internals |

---

## 2. Enforcement

| Strategy | Mechanism | Frequency |
| --- | --- | --- |
| Auto-label | `.github/workflows/architecture-review.yml` adds `architecture-review` label to PRs touching patterns above | Every PR |
| PR checklist | `.github/pull_request_template.md` includes architecture checklist | Every PR |
| Manual review | Compare diagrams in `docs/architecture/` against actual code structure | Quarterly |

---

## 3. Drift Detection Flow

```plain
Source of Truth (docs/architecture/, docs/adr/)
        ↕ compare
Actual Implementation (src-tauri/, src/)
        ↕ enforce
CI Pipeline (auto-label for review)
```

For small projects like Oh My VPN (single desktop app, solo developer), the auto-label + PR checklist combination provides sufficient coverage. Automated dependency graph comparison (e.g., `cargo modules`) can be added when the module count exceeds 10.

---
