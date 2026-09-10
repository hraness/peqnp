import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";
export const EXTRACTION_REPORT_SHA256 = "dbc3dc212c0464a242e5555387fb1bfe102733495caa357877030b134347a3cb";
export const EXTRACTION_EDITION_KEY = "edition:extraction-cost-" + EXTRACTION_REPORT_SHA256;
export const EXTRACTION_EVIDENCE_KEY = "evidence:extraction-cost-d68a7e03b40b7c7b528b68a3171acafe03b8fa95c6cfee6bd28d9fb6965183d9";

// Reviewed v5 argument: the extraction-cost lesson, bound to its exact
// experiment edition and evidence, and the next inquiry.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

const REVIEW = {
  reviewerKind: "agent",
  status: "independently-reviewed-argument",
  scope: "The measured-result interpretation was independently reviewed in the project; this specification does not constitute a kernel receipt or human review.",
};

export function researchRecordsV5() {
  return [
    record("statement:extraction-cost-v1-constant-factor-lesson", "statement", {
      proposition: "For the exact extraction-cost-v1 report bound below, all three arms decide every one of 176 fresh and 162 matched cases correctly with no exhausted budget, and the settled and per-literal arms derive identical unit lists with identical non-extraction phase counters and identical residual counters on every case. Settled extraction costs 158478 events against 243676 on the fresh corpus and 170204 against 256126 on the matched corpus, cheaper on all 92 fresh cases and 137 of 141 matched cases where extraction ran and 41 events dearer on four matched chain controls. Against the baseline the two extraction arms win and lose the same 84 and 92 fresh cases, 81 of the wins by fragment certificate; on the matched corpus the settled arm wins 27 against the per-literal arm's 22, the five additional wins being 12-variable random mixed cases with one to five forced literals. A constant-factor reduction of the read term therefore changed the sign of the interface delta on no fresh case and on five matched cases, and the settled arm's matched aggregate remains about 21 percent above the baseline.",
      assumptions: [
        "The existing extraction-cost edition and observation evidence have been validated and independently reviewed.",
        "Both extraction procedures have worst-case cost O(n(n+m_2)); no linear bound is claimed, per statement:two-cnf-backbone-not-known-linear-v1.",
        "The matched corpus was known before the protocol was written and is reported as secondary; the primary corpus was generated after the freeze.",
      ],
    }, ["context:research-method", "assertion:interface-delta-v1", "assertion:two-cnf-backbone-not-known-linear-v1"]),
    record("assertion:extraction-cost-v1-constant-factor-lesson", "assertion", {
      statement: "statement:extraction-cost-v1-constant-factor-lesson",
      stance: "bounded-inference",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, [EXTRACTION_EDITION_KEY, EXTRACTION_EVIDENCE_KEY, "statement:extraction-cost-v1-constant-factor-lesson"]),
    record("evidence:extraction-cost-v1-constant-factor-lesson", "evidence", {
      assertion: "assertion:extraction-cost-v1-constant-factor-lesson",
      kind: "bounded-result-interpretation",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Read the stored primary summary: 176 cases, 37 satisfiable; baseline 831346, per-literal 853853 with extraction 243676, settled 768655 with extraction 158478; settled_vs_baseline 84 better 92 worse, per_literal_vs_baseline 84 better 92 worse, settled_vs_per_literal 92 better 84 tied 0 worse; extraction_settled_vs_per_literal 92 better of 92 compared; settled stats 1361 searches, 286 inherited, 177 settled good.",
          "Read the stored secondary summary: 162 cases; per-literal 1245876 with extraction 256126 reproducing the fragment-interface-v1 artifact; settled 1159954 with extraction 170204; settled_vs_baseline 27 better 135 worse against 22 and 140; settled_vs_per_literal 137 better 21 tied 4 worse; the four worse cases are the 8-variable chain-embedded controls at 41 events each.",
          "Enumerate the fresh wins: 81 carry a fragment UNSAT certificate with zero residual; the three others are wins for both extraction arms. Enumerate the five matched flips: all are 12-variable random mixed cases whose settled total lies below the baseline and whose per-literal total lies above it.",
          "Hence the read term fell by about a third under an unchanged worst-case bound, and the delta's sign moved only where the per-literal loss was within about a third of its extraction cost, which happened on five matched cases and no fresh case; the largest such loss was 0.346 of its extraction cost.",
        ],
      },
      citations: [
        { title: "Dual Depth First Search for Binary Clause Reasoning", authors: "Sam Buss, Oliver Kullmann, Virginia Vassilevska Williams", url: "https://mathweb.ucsd.edu/~sbuss/ResearchWeb/DualDFS/DualDFS_SAT.pdf", role: "Why no linear bound is available for either procedure; the settled scheme is simpler than their DualDFS and inherits none of its analysis." },
      ],
      review: REVIEW,
      limitations: [
        "Finite constructed corpora of at most 12 variables and width three with a toy residual solver; not scaling evidence and not a ranking of solvers.",
        "Counts are declared operational events under one implementation and cost model; not elapsed time, memory, or a proved bit-cost bound.",
        "The result concerns two complete extraction procedures for one interface; it says nothing about general SAT or P versus NP.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:extraction-cost-v1-constant-factor-lesson", EXTRACTION_EDITION_KEY, EXTRACTION_EVIDENCE_KEY]),
    record("inquiry:solver-owned-interface-v6", "inquiry", {
      question: "Does an interface the residual solver already builds, such as its own watch lists or implication structure, expose the binary fragment's forced literals at a marginal read cost small enough that the interface delta is positive on mixed 2/3-CNFs whose fragment forces literals, now that a separately built graph with a read cost cut by a third changes the delta's sign on five matched cases and no fresh one?",
      status: "open",
      supersedes: "inquiry:constant-factor-extraction-and-solver-owned-interfaces-v5",
      designConstraint: "Freeze the protocol before the corpus exists, charge every structure the solver maintains inside the baseline as well as the interface arm so that construction is paid once, keep the extraction-cost-v1 corpora available for a matched comparison, preserve all six earlier artifacts unchanged, and page the ledger bundle through a reviewed change before admitting another artifact, since the bundle stands at about 13 MiB against the runtime's 16 MiB parse bound.",
      formalProofAccepted: false,
    }, ["assertion:extraction-cost-v1-constant-factor-lesson", "inquiry:constant-factor-extraction-and-solver-owned-interfaces-v5"]),
  ];
}
