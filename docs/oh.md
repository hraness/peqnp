# Canonical project research ledger

Oh is the source of truth for peqnp's research questions, claims, reviewed arguments, experiment observations, and narrative editions. The project versions its complete supported operation bundle in `ledger/operations.json`, with byte identity and replay verification in `ledger/manifest.json`. Other clones reconstruct the same history. The ignored `.oh/research.sqlite` database, space `peqnp`, is the local working materialization.

Research Markdown is a readable, checked view. Authors edit it, review it, and explicitly record new editions before updating the canonical bundle. Code and raw experiment artifacts remain executable evidence; Oh records their identities and interpretation. A valid ledger proves consistent record history, not mathematical truth.

## Restore a clone

Use Bun 1.3.14 and the checked lockfile:

```sh
bun install --frozen-lockfile --ignore-scripts
bun scripts/oh-ledger.mjs contract
bun run check:ledger
bun run oh:init
bun run oh:verify
```

`check:ledger` verifies the tracked bundle and manifest, replays every operation in a disposable database, and checks current narrative and report files against their latest canonical editions. It does not create or depend on `.oh`.

`oh:init` restores the tracked history; `bun run oh:restore` is the explicit equivalent. Restore validates the entire bundle before opening or creating the selected database. It never creates independent seed operations. Repeating an identical restore is a no-op. An exact local prefix can advance atomically. A compatible newer local history is preserved and reported as `local-ahead`: those additional operations have not been published in the tracked bundle. Divergent histories fail without changing the destination. Preserve both histories for explicit reconciliation.

The internal `initializeLedger` function is retained for isolated tests and the original bootstrap history. It is not the new-clone workflow. Recreating seeds would produce different operation timestamps and therefore a different history, even if record values matched.

## Record experiments and reviewed research

After generating and independently reviewing an experiment artifact, record the relevant observation:

```sh
bun run oh:record
bun run oh:record:transfer
bun run oh:record:indexed
bun run oh:record:implication
bun run oh:record:research
bun run oh:record:research:v2
bun run oh:verify
```

`oh:record` ingests `artifacts/calibration.json`; `oh:record:transfer` ingests `artifacts/clue-transfer.json`; `oh:record:indexed` ingests `artifacts/indexed-transfer.json`; `oh:record:implication` ingests `artifacts/implication-calibration.json`. Run the command for the observation being added. An explicit alternative repository-relative path is accepted by `bun scripts/oh-ledger.mjs record relative-report.json`, `record-transfer relative-report.json`, `record-indexed relative-report.json`, or `record-implication relative-report.json`, but the current publication profile in `scripts/oh-experiments.mjs` admits only the four named artifact paths. Extend the reviewed profile before publishing another experiment.

The calibration validator requires the two-variable, eight-program contract, complete coverage counters, selected survivor, rejected false-completeness control, and absence of a kernel proof. Transfer validation requires the fixed protocol, all 24 mining candidates and four accepted rules, 122 named evaluation cases, shared one-million-event budgets, valid finite outcomes, category sums, and consistent case/family/aggregate totals. It checks the report's protocol digest against the protocol bytes. It independently checks the tiny mining truth table, but does not reproduce held-out generation, labels, or measured solver costs. The experiment checks own those claims.

The indexed-transfer validator requires the fixed protocol digest, the same four accepted rules compiled to one schema, the 160 named fresh cases with their parameters and effective seeds, the 16 duplicate-pressure controls regenerated exactly, random-case clause counts and widths, complete reference coverage counters, shared one-million-event budgets for all three arms, category sums, decisions that agree with the reference label, unknown outcomes only on an exhausted budget, index counters consistent with the eligible binary clauses and derived units, and the protocol's generic/indexed equivalence label with equal derived units and residual counters wherever that label requires them. It recomputes the aggregate summary, every family summary, all three pairwise comparisons, and the setup-plus-online totals. The implication-calibration validator requires the fixed protocol digest, the same frozen library, the 120 named cases with the engineered families regenerated exactly and their stated backbone or UNSAT expectations, random-case shape, the reference backbone defined exactly when the model count is positive, budgets and category sums for the local, DPLL, and graph phases, decisions that agree with the reference, no clues or backbone claim on UNSAT rows, complete graph backbones equal to the reference and local units contained in it, and checker counters that match the reported certificates and clues. It re-checks every SAT certificate, UNSAT opposite-path certificate, and clue path directly against the raw clauses, then recomputes the summary, family summaries, coverage counters, the decision comparison, and the outside-arm totals. Neither validator reproduces held-out generation, truth-table labels, or measured event costs; the experiment checks own those claims.

The transfer, indexed-transfer, and implication observations preserve their negative or calibration results: each assertion is `bounded-observation`, and each comparison proposition asserts no performance improvement. Evidence includes the frozen rule array's canonical SHA-256, the numeric summary, and acquisition, compilation, or outside-arm costs; the implication evidence also carries the coverage counters and the decision-comparison declaration. Source hashes describe files observed at ingestion; they are not an attestation that those files generated the supplied report.

`oh:record:research` adds the reviewed definitions in `scripts/research-records.mjs`: signed-renaming soundness, the finite total-cost lesson, the longer binary-backbone family, the conditional strong-backdoor result, and the next inquiry. Their linked statement, assertion, and evidence records contain proof steps, assumptions, citations, limitations, and explicit informal or bounded status. Ingestion requires the full admitted history and the exact existing transfer edition/evidence dependency. Later source changes cannot silently retarget those arguments. Append versioned definitions when changing a reviewed argument or its evidence, and retain prior seed, experiment, and research constructors: historical operations must remain admissible and replayable.

`oh:record:research:v2` adds the reviewed definitions in `scripts/research-records-v2.mjs`: the interface-delta definition and its coNP-hardness reading, the indexed-transfer total-cost lesson, the implication-calibration coverage-and-decision lesson, and the next inquiry on expensive residuals. It requires the exact indexed-transfer and implication-calibration editions and evidence and the v1 research records to exist already.

Each observation/source identity gets additive records. Reingesting identical report bytes at the same path with identical source hashes is a no-op. Equal bytes at another path conflict with the original edition rather than rewriting provenance. Existing records cannot be replaced through these commands.

## Record narrative editions and publish the history

The v2 narrative set retains the original six v1 files (proposal, knowledge framework, clue-transfer theory, two experiment reports, and transfer protocol) and adds the indexed-transfer and implication-calibration protocols and reports. The v1 six-file constructor remains available for historical replay; v2 explicitly admits the ten-file set. After the protocols and reports are finalized and their edits reviewed:

```sh
bun run oh:record:docs
bun run oh:export
bun run check:ledger
bun run check
```

`oh:record:docs` records each Markdown body and its SHA-256, plus a versioned canonical-document registry. Unchanged current content is a no-op. Each changed registry references its predecessor, including across v1-to-v2 activation; reverting from document A to B and back to A activates the old A edition through a new registry without erasing B.

`oh:export` verifies the local operation history, reconstructs every historical record through the reviewed project constructors, checks current document/report parity, and requires the existing bundle to be an exact prefix. A matching profile label alone cannot admit an unrelated record. Every newly exported document edition must match the current Markdown unless it already appears in the existing local bundle. An intermediate edition that differs from the current files and is absent from that prior bundle is rejected. This is a content-consistency rule, not a detector of private information. An existing local export is not automatically reviewed or published. Preserve refused state and obtain a separate explicit review of its publication scope; these commands do not purge or rewrite it.

Review the entire new operation suffix, including historical values, before committing the updated bundle and manifest through the repository's normal review and CI workflow. Put private source material and unrelated notes in a different store. Never put credentials, personal documents, or absolute local paths into this project history. SQLite, WAL, and SHM files remain ignored and are not committed.

Each output file is replaced atomically, but the bundle/manifest pair is not a filesystem transaction. A crash between replacements creates a detectable mismatch. Preserve the files and database, inspect the interrupted operation, and recover a known reviewed pair; verification never resets or silently repairs state.

## Inspect canonical records locally

After restoring the ledger:

```sh
bun node_modules/@hraness/oh/dist/cli.js get inquiry:p-equals-np --db .oh/research.sqlite --space peqnp
bun node_modules/@hraness/oh/dist/cli.js list --kind evidence --limit 30 --db .oh/research.sqlite --space peqnp
bun node_modules/@hraness/oh/dist/cli.js search "clue transfer" --mode keyword --limit 10 --db .oh/research.sqlite --space peqnp
```

Confirm the database exists before raw CLI reads: Oh's generic CLI initializes a missing database. The adapter's verification command refuses missing state.

## Pinned contract and supported replication boundary

The dependency is the [immutable Oh v0.4.3 Release](https://github.com/hraness/oh/releases/tag/v0.4.3), published 7 September 2026. `package.json` and `bun.lock` pin its canonical release tarball. Release checksum and GitHub asset digest agree:

```text
4153298fad814910c28708d07ff13afa467adeb80012c8841598c809982a3a8e
```

The adapter checks compiled contract `oh.ontology.v1` and digest:

```text
e53ae573c2af417082be9f554d0f6f3e317f054daf745181f462608e3f622594
```

It uses the supported SDK graph envelope with application profile `peqnp.research-ledger.v1`. Values are detached through canonical JSON so SDK indexing and replay agree on property order; supplying unsorted object keys directly to this release can cause a search-document replay mismatch.

Writes verify replay, inspect conflicts, and commit against the reviewed head. Document activation and research admission also bind their earlier inspection head. Stale-head conflicts fail without a retry loop adopting another writer's state.

Export uses `store.exportOperations` and `createOhSyncBundleV1`, requires complete genesis-to-head coverage, and independently replays the result. The bounded adapter refuses more than 1,000 operations or an 8 MiB output; it never silently truncates. A larger history needs reviewed pagination. Restore uses `store.importOperations({ expectedHead, operations })`, which validates and imports the bounded interval atomically in pinned v0.4.3. The native CLI uses this same atomic method in this release, while its export can return a largest-fitting prefix. The packaged skill's v0.4.2 installation paragraph and sequential-import warning are stale for this qualified runtime.

Hosted sync, imports from another authority, tombstones, hosted embeddings, and provisioning remain outside this workflow. Git transports the reviewed project history; no additional service or credentials are required.

## Focused checks

```sh
bun run check:oh
bun run check:ledger
```

Tests cover the four experiment contracts and additive writes, fresh-clone exact replay, idempotence, prefix and divergent histories, tampering, missing/partial manifests, path guards, document updates/reverts, allowlist admission of the newer experiments, and rejection of unrelated, forged, or hidden intermediate records. Test databases are disposable and never constitute research evidence. The final repository gate separately checks all three protocol digests and reproduces the four experiment artifacts byte for byte.
