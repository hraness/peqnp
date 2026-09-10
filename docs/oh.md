# Canonical project research ledger

Oh is the source of truth for peqnp's research questions, claims, reviewed arguments, experiment observations, and narrative editions. The project versions its complete supported operation history as a sequence of pages, `ledger/pages/0001.json` onward, each a complete `oh.sync.v1` bundle for one contiguous interval, with byte identity, page layout, and replay verification in `ledger/manifest.json` (schema `peqnp.public-ledger.v2`). Pages replace the former single `ledger/operations.json` file. Other clones reconstruct the same history. The ignored `.oh/research.sqlite` database, space `peqnp`, is the local working materialization.

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

`check:ledger` classifies the ledger layout, verifies every page's bytes and bundle digest against the manifest, requires `ledger/pages` to hold exactly the listed files and the pages to chain from genesis, replays the concatenation in a disposable database with one atomic import per page, and checks current narrative and report files against their latest canonical editions. It does not create or depend on `.oh`, and it ignores the scratch directory `ledger/.stage/`. A `check` that is interrupted or exceeds the CI time limit yields no verdict: rerun it.

`oh:init` restores the tracked history; `bun run oh:restore` is the explicit equivalent. Restore runs the full check, including a disposable replay of every page, before opening or creating the selected database. It never creates independent seed operations. Repeating an identical restore is a no-op. An exact local prefix advances by importing the missing suffix with one atomic `importOperations` call per page slice, each against the head verified after the previous call; a crash between calls leaves an exact prefix that the next restore fast-forwards. A compatible newer local history is preserved and reported as `local-ahead`: those additional operations have not been published in the tracked pages. Divergent histories fail without changing the destination. Preserve both histories for explicit reconciliation.

The internal `initializeLedger` function is retained for isolated tests and the original bootstrap history. It is not the new-clone workflow. Recreating seeds would produce different operation timestamps and therefore a different history, even if record values matched.

## Record experiments and reviewed research

After generating and independently reviewing an experiment artifact, record the relevant observation:

```sh
bun run oh:record
bun run oh:record:transfer
bun run oh:record:indexed
bun run oh:record:implication
bun run oh:record:fragment
bun run oh:record:extraction
bun run oh:record:research
bun run oh:record:research:v2
bun run oh:record:research:v3
bun run oh:record:research:v4
bun run oh:record:research:v5
bun run oh:verify
```

`oh:record` ingests `artifacts/calibration.json`; `oh:record:transfer` ingests `artifacts/clue-transfer.json`; `oh:record:indexed` ingests `artifacts/indexed-transfer.json`; `oh:record:implication` ingests `artifacts/implication-calibration.json`; `oh:record:fragment` ingests `artifacts/fragment-interface.json`; `oh:record:extraction` ingests `artifacts/extraction-cost.json`. Run the command for the observation being added. An explicit alternative repository-relative path is accepted by `bun scripts/oh-ledger.mjs record relative-report.json`, `record-transfer relative-report.json`, `record-indexed relative-report.json`, `record-implication relative-report.json`, `record-fragment relative-report.json`, or `record-extraction relative-report.json`, but the current publication profile in `scripts/oh-experiments.mjs` admits only the six named artifact paths. Extend the reviewed profile before publishing another experiment.

The calibration validator requires the two-variable, eight-program contract, complete coverage counters, selected survivor, rejected false-completeness control, and absence of a kernel proof. Transfer validation requires the fixed protocol, all 24 mining candidates and four accepted rules, 122 named evaluation cases, shared one-million-event budgets, valid finite outcomes, category sums, and consistent case/family/aggregate totals. It checks the report's protocol digest against the protocol bytes. It independently checks the tiny mining truth table, but does not reproduce held-out generation, labels, or measured solver costs. The experiment checks own those claims.

The indexed-transfer validator requires the fixed protocol digest, the same four accepted rules compiled to one schema, the 160 named fresh cases with their parameters and effective seeds, the 16 duplicate-pressure controls regenerated exactly, random-case clause counts and widths, complete reference coverage counters, shared one-million-event budgets for all three arms, category sums, decisions that agree with the reference label, unknown outcomes only on an exhausted budget, index counters consistent with the eligible binary clauses and derived units, and the protocol's generic/indexed equivalence label with equal derived units and residual counters wherever that label requires them. It recomputes the aggregate summary, every family summary, all three pairwise comparisons, and the setup-plus-online totals. The implication-calibration validator requires the fixed protocol digest, the same frozen library, the 120 named cases with the engineered families regenerated exactly and their stated backbone or UNSAT expectations, random-case shape, the reference backbone defined exactly when the model count is positive, budgets and category sums for the local, DPLL, and graph phases, decisions that agree with the reference, no clues or backbone claim on UNSAT rows, complete graph backbones equal to the reference and local units contained in it, and checker counters that match the reported certificates and clues. It re-checks every SAT certificate, UNSAT opposite-path certificate, and clue path directly against the raw clauses, then recomputes the summary, family summaries, coverage counters, the decision comparison, and the outside-arm totals. Neither validator reproduces held-out generation, truth-table labels, or measured event costs; the experiment checks own those claims.

The fragment-interface validator requires the fixed protocol digest, no formal-proof claim, the shared one-million-event budget, and the pinned counter model, corpus, comparison, and encoding declarations. It regenerates the 162-case corpus identity (36 ternary-only, 108 mixed, 12 chain-embedded, and 6 contradictory-fragment cases with their parameters and effective seeds, no duplicate declared-variables and ordered-CNF pair), regenerates the chain and parity-cycle prefixes exactly, and checks the pseudorandom clauses for count, width, and distinct variables only. Per case it requires category sums for every phase, arm totals equal to the sum of their phases and within budget, unknown outcomes only on an exhausted budget with no certificate, clue, or unit, completed decisions that agree with the reference, a reference backbone defined exactly when the model count is positive, and complete truth-table coverage counters. It re-checks every fragment SAT certificate against the width-at-most-two clauses, every UNSAT certificate and clue path step by step against the cited raw clauses (each of width at most two, simple, with the claimed endpoints), derived units equal to the clue literals in order and contained in the reference backbone on satisfiable cases, chain controls deriving exactly the selected sign, contradictory controls decided UNSAT by the fragment alone with no extraction, append, or residual work, ternary-only controls deriving nothing with residual counters equal to the baseline's work, and checker counters that match the reported certificate and clues. It then recomputes the summary, every family, binary-count, variable, and density subtotal, the fragment-versus-baseline comparison, the outside-arm totals, and the enumeration ratios. It does not reproduce the pseudorandom generation, the truth-table labels, or the measured event costs; the experiment checks own those claims.

The extraction-cost validator requires the fixed protocol digest, no formal-proof claim, the shared one-million-event budget, and the pinned counter model, corpus, arm, comparison, and encoding declarations. It regenerates the 338-case corpus identity: the 144 random mixed and 24 fragment-only primary cases by their parameters and effective seeds, the 8 ladder controls exactly from the protocol's construction, and the 162 secondary cases through the fragment-interface validator's own corpus builder under `v1:` ids and families, with no duplicate declared-variables and ordered-CNF pair across all 338. Per case it applies the fragment-interface arm checks to both the per-literal and the settled arm (category sums for every phase, totals within budget, unknown only on an exhausted budget with no certificate, clue, or unit, completed decisions that agree with the reference, a reference backbone defined exactly when the model count is positive, complete coverage counters, every certificate and clue path re-checked against the raw clauses, derived units equal to the clue literals and contained in the reference backbone on satisfiable cases, and the v1 chain, contradictory, and ternary-only control expectations). It requires ladder rows to derive exactly the j ladder literals and no y literal, settled statistics present exactly when the settled extraction completed with searches plus inherited vertices at most 2n, settled-good vertices at most 2n, and visited entries equal to searches plus extraction search nodes, and, whenever both fragment arms completed, the same outcome, fragment certificate, and ordered derived units with identical construction, components, certificate, append, and residual counters, so the two arms differ in extraction alone. It then recomputes the primary and secondary summaries, every family and binary-count subtotal, the three total-work comparisons and the extraction-only comparison, the search and settled-statistics totals, the enumeration ratios by the same integer rounding, and the outside-arm totals. It does not reproduce the pseudorandom generation, the truth-table labels, the measured event costs, or the `arm2_reproduces_v1` claim that the per-literal arm matched the fragment-interface artifact counter for counter; the experiment checks own those claims.

Validators for protocols above the 12-variable truth-table bound share `oracleReference(reference, row, pins, limits, answers)` in `scripts/oh-report-checks.mjs`; the five frozen validators do not use it. It accepts a `truth-table` row only at or below 12 variables with the existing coverage and backbone checks, a `definition` row only above it with its empty-clause index or all-false model recomputed from the raw clauses, and an `oracle` row only when its `dimacs_sha256` equals the digest of the DIMACS text the validator regenerates from the raw clauses with the exact `write_dimacs` layout, its conflict limit and oracle identity equal the protocol's and the pins, and the committed oracle-answer file `artifacts/<experiment>-oracle.json` holds an entry with that digest and label. A `sat` row's model must be ascending, complete, satisfy every clause, and equal the committed model; an `unsat` row's `lrat_sha256` and `lrat_bytes` must equal the digest and length of the committed proof text, which the Rust replay re-checks on every gate run while the validator does not re-run propagation; an `unknown` row carries a reason and no certificate. Every `RatioPair` must carry the thousandths recomputed with `BigInt`. The answer file is recorded as an edition, `edition:<prefix>-oracle-<sha256 of file bytes>`, that the evidence record depends on; the evidence carries `oracleModel`, `oracleCalls`, `oracleUnknown`, `proofLemmasTotal`, `answersSha256`, and the sentence "a checked model and a checked LRAT proof certify the pinned solver's answer on that formula; they prove nothing about any arm and nothing asymptotic". The timing sidecar `artifacts/<experiment>-timing.json` is uncommitted and never recorded.

The transfer, indexed-transfer, implication, fragment-interface, and extraction-cost observations preserve their negative or calibration results: each assertion is `bounded-observation`, and each comparison proposition asserts no performance improvement. Evidence includes the frozen rule array's canonical SHA-256 where the experiment uses a library, the numeric summary, and acquisition, compilation, or outside-arm costs; the implication evidence also carries the coverage counters and the decision-comparison declaration, the fragment-interface evidence carries the phase totals, the comparison declaration, and the descriptive enumeration ratios, and the extraction-cost evidence carries the primary and secondary summaries, the three total-work comparisons and the extraction-only comparison, the extraction phase totals of both fragment arms, the settled-statistics totals, the outside-arm totals, and the enumeration ratios of both sections. Source hashes describe files observed at ingestion; they are not an attestation that those files generated the supplied report.

The pinned runtime caps one record value at 1 MiB of canonical JSON. A report that fits is stored whole in its edition, exactly as before. The extraction-cost report (1.86 MB on disk, 1.78 MB as canonical JSON) is stored as the same edition without its observations plus ordered observation-part editions, `edition:extraction-cost-<sha256>-part-<index>`, that the edition depends on; publication admission and the current-report parity check reassemble the exact report from those parts before revalidating it. The report-file bound in `scripts/oh-ledger.mjs` was raised in a reviewed step to 4 MiB for the same experiment; the history bytes are bounded per page, as the pinned-contract section states.

`oh:record:research` adds the reviewed definitions in `scripts/research-records.mjs`: signed-renaming soundness, the finite total-cost lesson, the longer binary-backbone family, the conditional strong-backdoor result, and the next inquiry. Their linked statement, assertion, and evidence records contain proof steps, assumptions, citations, limitations, and explicit informal or bounded status. Ingestion requires the full admitted history and the exact existing transfer edition/evidence dependency. Later source changes cannot silently retarget those arguments. Append versioned definitions when changing a reviewed argument or its evidence, and retain prior seed, experiment, and research constructors: historical operations must remain admissible and replayable.

`oh:record:research:v2` adds the reviewed definitions in `scripts/research-records-v2.mjs`: the interface-delta definition and its coNP-hardness reading, the indexed-transfer total-cost lesson, the implication-calibration coverage-and-decision lesson, and the next inquiry on expensive residuals. It requires the exact indexed-transfer and implication-calibration editions and evidence and the v1 research records to exist already.

`oh:record:research:v3` adds the reviewed definitions in `scripts/research-records-v3.mjs`: Levin's pointer-versus-compression distinction as the certificate asymmetry of NP, with Poincaré's examiner as the clue pre-filter; the fragment-interface first-positive-delta lesson; and the next inquiry on linear extraction and solver-owned interfaces. It requires the v2 records and the exact fragment-interface edition and evidence to exist already.

`oh:record:research:v4` adds the reviewed literature finding in `scripts/research-records-v4.mjs`: the complete 2-CNF backbone is not known to be computable in linear time, by the Buss, Kullmann, and Vassilevska Williams reduction from triangle detection; it withdraws an earlier report sentence and supersedes the v4 inquiry with `inquiry:constant-factor-extraction-and-solver-owned-interfaces-v5`. It requires the v3 records to exist already.

`oh:record:research:v5` adds the reviewed definitions in `scripts/research-records-v5.mjs`: the extraction-cost constant-factor lesson, bound to the exact extraction-cost edition and evidence, and `inquiry:solver-owned-interface-v6`. It requires the v4 records and that edition and evidence to exist already.

Each observation/source identity gets additive records. Reingesting identical report bytes at the same path with identical source hashes is a no-op. Equal bytes at another path conflict with the original edition rather than rewriting provenance. Existing records cannot be replaced through these commands.

## Record narrative editions and publish the history

The v4 narrative set is fourteen files: the original six v1 files (proposal, knowledge framework, clue-transfer theory, two experiment reports, and transfer protocol), the indexed-transfer and implication-calibration protocols and reports added by v2, the fragment-interface protocol and report added by v3, and the extraction-cost protocol and report. The v1 six-file, v2 ten-file, and v3 twelve-file constructors remain available for historical replay; v4 explicitly admits the fourteen-file set, and the current-parity check compares working Markdown against a v4 registry. After the protocols and reports are finalized and their edits reviewed:

```sh
bun run oh:record:docs
bun run oh:export
bun run check:ledger
bun run check
```

`oh:record:docs` records each Markdown body and its SHA-256, plus a versioned canonical-document registry. Unchanged current content is a no-op. Each changed registry references its predecessor, including across v1-to-v2, v2-to-v3, or v3-to-v4 activation; reverting from document A to B and back to A activates the old A edition through a new registry without erasing B.

`oh:export` verifies the local operation history, reconstructs every historical record through the reviewed project constructors, checks current document/report parity, and requires the committed pages to be an exact prefix. A matching profile label alone cannot admit an unrelated record. Every newly exported document edition must match the current Markdown unless it already appears in the committed pages. An intermediate edition that differs from the current files and is absent from that prior history is rejected. This is a content-consistency rule, not a detector of private information. An existing local export is not automatically reviewed or published. Preserve refused state and obtain a separate explicit review of its publication scope; these commands do not purge or rewrite it.

New operations are appended by one rule: an operation joins the open (last) page while the rebuilt page stays within `pageSoftBytes` (4,194,304 bytes); otherwise the open page is sealed and a new page opens with that operation. A candidate of exactly the soft bound is accepted. A one-operation page may exceed the soft bound but never `pageHardBytes` (8,388,608 bytes); an operation whose one-operation page would exceed it is refused before any write, and its ingestion must be split. Sealed pages are immutable bytes that export never rewrites; the open page changes only by appending, so its listed interval stays an exact prefix and an append diffs as added lines plus the bundle digest. The manifest records the layout used by the export that wrote it and lists every page with its byte count, file SHA-256, bundle digest, first and last sequence, and last operation digest.

Files are staged under `ledger/.stage/` and renamed into place in ascending page order, the manifest last. An interruption before the manifest write leaves an unlisted page or a grown open page; `check:ledger` reports it as `interrupted export; rerun oh:export`, and the rerun verifies that every sealed page is unchanged, that the listed interval of the open page is still an exact prefix, and that every pending operation on disk equals the local operation of the same sequence, then writes only the remaining files. The paging rule is deterministic, so the rerun regenerates the same bytes as an uninterrupted export; a second export on an unchanged local history writes nothing (`written: []`).

The single-file layout (`peqnp.public-ledger.v1`, `ledger/operations.json`) is migrated once with `bun run oh:migrate`, which never touches `.oh`. It pages the legacy operations from genesis, writes `ledger/pages`, writes the v2 manifest with a `migration.legacy` record of the legacy file's digests, verifies, unlinks the legacy file, verifies again, and writes the final manifest without `migration`. Every intermediate state (pages beside a v1 manifest; a v2 manifest with `migration` beside or without the legacy file) is accepted by `check:ledger`, refused by `oh:export` with `finish oh:migrate`, and completed by rerunning `oh:migrate`, which reports `already-migrated` once the final layout is in place.

Review the entire new operation suffix, including historical values, before committing the updated bundle and manifest through the repository's normal review and CI workflow. Put private source material and unrelated notes in a different store. Never put credentials, personal documents, or absolute local paths into this project history. SQLite, WAL, and SHM files remain ignored and are not committed.

Each output file is replaced atomically, but the pages and manifest are not one filesystem transaction. A crash between renames is the detectable interrupted state above, completed by the next export; any other mismatch (a changed sealed page, a page that does not chain, a legacy file beside a final manifest) is refused as tampering or an unrecognized layout. Preserve the files and database, inspect, and recover a known reviewed layout; verification never resets or silently repairs state.

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

Export uses `store.exportOperations` and builds each page with `createOhSyncBundleV1`, which accepts any contiguous chain; genesis coverage and the chain across page boundaries are this repository's rules. The concatenation is independently replayed. The bounded adapter refuses more than 1,000 operations in the whole history (the retained review bound), a page file above 8,388,608 bytes, a manifest above 65,536 bytes, and a legacy single file above 16,777,216 bytes during migration; the soft page bound is 4,194,304 bytes. It never silently truncates. These bounds sit far under the runtime's own: 1,000 operations per bundle, 64 MiB + 4 KiB of canonical bytes per bundle, 64 MiB per operation, and a 16 MiB default for canonical-text parsing, which the page path does not use because it parses objects. Restore uses `store.importOperations({ expectedHead, operations })`, which validates and imports one page interval atomically in pinned v0.4.3; a stale `expectedHead` is refused. The native CLI uses this same atomic method in this release, while its export can return a largest-fitting prefix. The packaged skill's v0.4.2 installation paragraph and sequential-import warning are stale for this qualified runtime.

Hosted sync, imports from another authority, tombstones, hosted embeddings, and provisioning remain outside this workflow. Git transports the reviewed project history; no additional service or credentials are required.

## Focused checks

```sh
bun run check:oh
bun run check:ledger
```

Tests cover the six experiment contracts and additive writes, fresh-clone exact replay across pages with identical re-export bytes, idempotence, the append and open-page rules including the exact soft-bound tie, prefix restore with one atomic import per page and a simulated crash between imports, prefix and divergent histories at page boundaries, every paged failure mode with `.oh` absent and files byte-identical afterwards, oversized-operation refusal, interrupted exports resumed after zero, one, and two renames, the one-time migration with each interrupted state, a leftover `ledger/.stage`, missing/partial manifests, path guards, document updates/reverts across registry versions, allowlist admission of the newer experiments, and rejection of unrelated, forged, or hidden intermediate records. Test pages are tiny synthetic layouts; test databases are disposable and never constitute research evidence. The final repository gate separately checks all five protocol digests and reproduces the six experiment artifacts byte for byte.
