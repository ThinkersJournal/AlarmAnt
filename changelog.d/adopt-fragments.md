kind: tooling: changelog fragments and version stamp
---
- Adopt the portfolio changelog-fragment workflow: `changelog.d/` fragments, `tools/stamp-version.py` (run by the gate at merge), `tools/check-fragment.py` and the `changelog-fragment` CI check. PRs now add one fragment and never edit CHANGELOG.md, Cargo.toml or Cargo.lock versions.
