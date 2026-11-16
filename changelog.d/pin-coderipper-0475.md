kind: changed
---
CI: the shared portfolio workflow moves to coderipper c72a6e03 (CodeRipper 0.47.5). No rule names to migrate: the repository has no `.coderipper.toml` and names no rule in any file (the migration tool exits 2 on the missing config; a grep for the 14 old names finds none). Verified by reading the Portfolio job of this PR.
