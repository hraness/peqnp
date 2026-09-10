# Local research ledger

Oh records the investigation's questions, claims, experiment observations, and their dependencies. The local ledger lives at `.oh/research.sqlite` in space `peqnp`. It is ignored by Git. Reviewed experiment JSON and explanatory documents are the public, reproducible record; database operation history remains local.

## Install and initialize

Use Bun 1.3.14 and the checked lockfile:

```sh
bun install --frozen-lockfile --ignore-scripts
bun node_modules/@hraness/oh/dist/cli.js version
bun scripts/oh-ledger.mjs contract
bun run oh:init
bun run oh:verify
```

Only `oh:init` may create the selected database. Verification refuses a missing database. Initialization commits six seed records atomically: the research context, open inquiry, P = NP proposition and hypothesis stance, and the unit-propagation proposition and calibration target. It asserts no new mathematical result.

Rerunning initialization is an exact-content no-op. If an existing seed has different content, initialization refuses the batch and leaves the existing records intact. It never overwrites, repairs, deletes, imports, or syncs a store.

## Record a calibration result

After running the Rust experiment, record its default artifact:

```sh
cargo run --locked --release -- experiment artifacts/calibration.json
bun run oh:record
bun run oh:verify
```

An alternative result must be a repository-relative JSON file:

```sh
bun scripts/oh-ledger.mjs record experiments/calibration.json
```

The `record` command accepts only the bounded `unit-propagation-calibration-v1` envelope. It validates the declared two-variable corpus, eight distinct candidate programs, coverage counters, selected survivor, rejected false-completeness control, and explicit absence of a kernel-checked proof. It records the exact file SHA-256, JSON payload, source-file hashes observed at ingestion, and a `bounded-tested` assertion with supporting evidence.

Validation of a report's shape does not independently reproduce its observations. Source hashes describe the files at ingestion; they do not attest that those files generated the supplied report. The experiment and independent checker own that evidence. The ledger neither upgrades a finite test to a general proof nor interprets an informal argument as a formal proof receipt.

Each report/source identity gets an additive activity, assertion, and evidence record. Reingesting identical bytes at the same relative path with identical source hashes is a no-op. The content edition retains its first path; copying identical bytes to another path causes an explicit conflict, not a provenance rewrite. A later source snapshot produces a distinct observation; it does not rewrite the old result.

## Record a clue-transfer comparison

After producing and independently reviewing the [clue-transfer report](../artifacts/clue-transfer.json), record it in the same local ledger:

```sh
bun run oh:record:transfer
bun run oh:verify
```

For an alternative repository-relative report path, use `bun scripts/oh-ledger.mjs record-transfer relative-report.json`. Initialization remains an explicit prerequisite.

This command accepts the fixed `clue-transfer-v1` protocol: 24 implication candidates on the six training pairs, four frozen rule records, and all 122 named evaluation cases with at most 12 declared variables. It checks the complete two-variable mining conclusions, the declared one-million-event budget for each arm, valid finite inputs and outcomes, event-category sums, reference-coverage counters, and aggregate/family totals recomputed from each observation. It also verifies the report's protocol digest against the checked-in protocol bytes. Shape and arithmetic checks do not establish that the submitted inputs were generated as declared or that measured costs and labels were produced by the reported source; the independent experiment validation owns those claims.

The first ingestion adds five records: a comparison proposition, an exact report edition, a source-observation activity, a `bounded-observation` assertion, and evidence. The evidence includes the canonical SHA-256 of the frozen rule array, numeric summary, acquisition cost, and acquisition-plus-online cost. The proposition describes the finite comparison and asserts no performance improvement. Negative results and resource exhaustion retain their reported meaning; no formal proof is accepted. Repeated ingestion follows the same identity and conflict rules as the calibration command.

This workflow only writes `.oh/research.sqlite`. The live database and operation history remain local and ignored; no export or public ledger snapshot is part of these commands.

## Inspect the record

```sh
bun node_modules/@hraness/oh/dist/cli.js get inquiry:p-equals-np --db .oh/research.sqlite --space peqnp
bun node_modules/@hraness/oh/dist/cli.js list --kind evidence --limit 20 --db .oh/research.sqlite --space peqnp
bun node_modules/@hraness/oh/dist/cli.js search "unit propagation" --mode keyword --limit 10 --db .oh/research.sqlite --space peqnp
```

Inspect the database's existence before raw CLI reads because Oh's generic CLI opens a missing database as part of normal operation. The adapter's `verify` command makes this check itself.

## Pinned contract and write boundary

The dependency is the [immutable Oh v0.4.3 Release](https://github.com/hraness/oh/releases/tag/v0.4.3), published 7 September 2026. The canonical release tarball is pinned directly in `package.json` and `bun.lock`. Its SHA-256 is:

```text
4153298fad814910c28708d07ff13afa467adeb80012c8841598c809982a3a8e
```

The release's `SHA256SUMS` and GitHub asset digest agree. Runtime qualification reports CLI version `0.4.3`, contract `oh.ontology.v1`, and contract digest:

```text
e53ae573c2af417082be9f554d0f6f3e317f054daf745181f462608e3f622594
```

The packaged skill still names v0.4.2 in its installation paragraph. This integration uses the separately verified immutable v0.4.3 artifact and checks its compiled contract before opening the ledger.

The adapter uses Oh's supported SDK graph envelope and an explicit application profile, `peqnp.research-ledger.v1`; it does not claim the application values are native ontology-codec objects. It detaches values through canonical JSON before commit. This also keeps SDK keyword indexing and replay on the same property order: qualification found that supplying unsorted object keys directly to this release can produce a search-document replay mismatch.

Every write batch verifies replay, captures the exact current generation and operation digest, checks for conflicting records, and commits with compare-and-swap. Stale-head conflicts fail; there is no retry loop that silently adopts another writer's state. Replay verification runs again after the batch. Treat a verification failure as evidence to investigate, preserving the database.

Remote sync, imports, tombstones, hosted embeddings, and publishing operation bundles are outside this adapter. No credentials or private source documents belong in the public research artifacts.

## Focused checks

```sh
bun run check:oh
```

Tests cover non-creating reads, repeatable bootstrap, atomic rejection of conflicting records, both bounded report contracts, summary and cost consistency, idempotent ingestion, proof-status preservation, protocol identity, and path boundaries. Test databases are disposable and never constitute research evidence.
