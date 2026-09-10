import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";
export const INDEXED_REPORT_SHA256 = "a95c7144abe2d65beb1fa69c288034b3be4aa1492bcdfbc12d33624e5936ac62";
export const INDEXED_EDITION_KEY = "edition:indexed-transfer-" + INDEXED_REPORT_SHA256;
export const INDEXED_EVIDENCE_KEY = "evidence:indexed-transfer-ade665dddf92363bb45728d6014ede1e190c31f10548842f6f65b2b8e94ae812";
export const IMPLICATION_REPORT_SHA256 = "41d7c2f40a13bdab812bd7ae1ecbf1150791b1649b5af46bbdf4c2fea84107f7";
export const IMPLICATION_EDITION_KEY = "edition:implication-calibration-" + IMPLICATION_REPORT_SHA256;
export const IMPLICATION_EVIDENCE_KEY = "evidence:implication-calibration-4c9ec92b42c6149abadff098f9c8f253b866a276d02ceb073537a7c01bb95676";

// Reviewed v2 arguments. Each retains its exact experiment edition/evidence
// dependency; revise the keys when an argument or its evidence changes.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

const REVIEW = {
  reviewerKind: "agent",
  status: "independently-reviewed-argument",
  scope: "The definition, mathematical argument, and measured-result interpretation were independently reviewed in the project; this specification does not constitute a kernel receipt or human review.",
};

export function researchRecordsV2() {
  return [
    record("statement:interface-delta-v1", "statement", {
      proposition: "For an input F, a representation R, and a fixed deterministic consumer D, define the interface delta Delta(F,R)=T_baseline(F)-[T_build(F->R)+T_read(R)+T_residual(D,F,clues read from R)] in one declared cost model, with one-time rule acquisition reported separately. Entailed information such as a backbone literal is determined by F, yet its extraction cost depends on R: for arbitrary CNF F with fresh x, x is a backbone of G=AND_{C in F}(x or C) exactly when F is unsatisfiable, so exact backbone extraction from the clause-list representation is coNP-hard, whereas the implication graph of a 2-CNF exposes every forced literal through a path from not l to l and admits a linear-time decision. A representation is promoted only with Delta>0 on a fresh protocol-frozen corpus and a proved polynomial bound on T_build and T_read; any contribution outside the declared cost model cannot appear in a complexity claim.",
      assumptions: [
        "Classical Boolean semantics and the standard uniform deterministic worst-case target for P versus NP.",
        "All four cost terms are measured in the same declared event model on the same input; amortized acquisition is reported only over an explicit batch.",
        "The definition is a measurement rule for comparing representations; it asserts nothing about which representations exist for general SAT.",
      ],
    }, ["context:research-method"]),
    record("assertion:interface-delta-v1", "assertion", {
      statement: "statement:interface-delta-v1",
      stance: "informal-argument-reviewed",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["statement:interface-delta-v1"]),
    record("evidence:interface-delta-v1", "evidence", {
      assertion: "assertion:interface-delta-v1",
      kind: "reviewed-definition-and-argument",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Michael Levin's Ingressing Minds proposes that physical systems and algorithms are interfaces into a space of patterns, that a good interface returns more than was put in, and that this delta between effort and competency should be quantified, with the framework dropped if no delta beyond conventional accounting is ever found.",
          "Restricted to computation, the free lunch is entailment: the third angle of a triangle and a NAND truth table are determined by their inputs and have short derivations in the representation at hand. A backbone literal of a CNF is likewise determined by the formula.",
          "The G construction shows determination does not make extraction cheap: G is satisfiable by x=true, and x=false leaves exactly F, so x is forced exactly when F is unsatisfiable. Deciding backbone membership on satisfiable CNFs is therefore coNP-hard by the standard SAT completeness argument.",
          "The same information can be cheap in another representation: in the implication graph of a 2-CNF, l is forced exactly when a path leads from not l to l, and strongly connected components decide satisfiability in linear time. The cost of a clue is a property of the interface, not of the clue.",
          "Hence the delta is defined as baseline work minus build, read, and residual work under one cost model. P versus NP asks whether one uniform interface makes every polynomially checkable entailment cheap on every input; that universal claim is the open problem and is never assumed.",
          "Levin's separate argument that formal models never fully capture a system is inapplicable to a complexity claim, whose cost model must be closed. The connection is a search heuristic and a measurement rule, not a proof step, and the article's metaphysics of non-physical or agential patterns is neither adopted nor tested here.",
        ],
      },
      citations: [
        { title: "Ingressing Minds: Causal, Non-Physical Patterns In-Form Natural, Synthetic, and Hybrid Embodiments", authors: "Michael Levin", url: "https://doi.org/10.3390/philosophies11050161", role: "Source of the interface and free-lunch framing and the effort-versus-competency delta; the computational reading is a project translation." },
        { title: "Classical Sorting Algorithms as a Model of Morphogenesis", authors: "Taining Zhang, Adam Goldstein, Michael Levin", url: "https://arxiv.org/abs/2401.05375", role: "Minimal example of deterministic algorithms showing behavior permitted but not prescribed by their steps." },
        { title: "A linear-time algorithm for testing the truth of certain quantified Boolean formulas", authors: "Bengt Aspvall, Michael F. Plass, Robert Endre Tarjan", url: "https://doi.org/10.1016/0020-0190(79)90002-4", role: "Primary source for the implication-graph interface and linear-time 2-SAT decision." },
        { title: "The P versus NP problem", authors: "Stephen Cook", url: "https://www.claymath.org/wp-content/uploads/2022/06/pvsnp.pdf", role: "Standard problem statement and target definition." },
      ],
      review: REVIEW,
      limitations: [
        "The coNP-hardness step is an elementary deduction from standard definitions, not a new theorem.",
        "The delta compares two implementations under one declared event model; it is not elapsed time, not a bit-cost bound, and not a ranking of competitive solvers.",
        "The framing motivates which representations to test and how to score them; it supplies no representation for general SAT.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:interface-delta-v1"]),
    record("statement:indexed-transfer-v1-total-cost-lesson", "statement", {
      proposition: "For the exact indexed-transfer-v1 report bound below, all 160 fresh cases are decided correctly by the baseline and indexed arms and 156 by the generic arm, whose four unknowns are preprocessing exhaustions. The indexed matcher derives the same units in the same order as the generic matcher on every case both complete, spends 1048871 preprocessing events against the generic matcher's 11235212, and is cheaper than the generic arm on 106 of 156 comparable cases; yet its total online work of 1452179 exceeds the 520161 baseline on every one of 160 cases, because its preprocessing exceeds the 116853 events of residual search it removes. Cheaper lookup of the same two-clause clues therefore does not change the v1 negative result on this corpus, and no batch size amortizes the 1089-event setup against the baseline.",
      assumptions: [
        "The existing indexed-transfer edition and observation evidence have been validated and independently reviewed.",
        "Same frozen library, same deterministic DPLL, same event model, one root preprocessor pass, and one million total events per arm including preprocessing.",
        "The corpus was generated after the protocol freeze from parameter-specific seeds and is exactly fresh relative to the v1 corpus; the 16 duplicate-pressure controls are engineered mechanism checks reported separately.",
      ],
    }, ["context:research-method"]),
    record("assertion:indexed-transfer-v1-total-cost-lesson", "assertion", {
      statement: "statement:indexed-transfer-v1-total-cost-lesson",
      stance: "bounded-inference",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, [INDEXED_EDITION_KEY, INDEXED_EVIDENCE_KEY, "statement:indexed-transfer-v1-total-cost-lesson"]),
    record("evidence:indexed-transfer-v1-total-cost-lesson", "evidence", {
      assertion: "assertion:indexed-transfer-v1-total-cost-lesson",
      kind: "bounded-result-interpretation",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Read the stored report summary: baseline complete 160, generic complete 156 with 4 unknown, indexed complete 160; indexed_vs_baseline both_complete 160 with left_better 0, tied 0, left_worse 160; indexed_vs_generic both_complete 156 with left_better 106, left_worse 50, left_complete_only 4.",
          "Preprocessing work is 1048871 (indexed) against 11235212 (generic); residual work is 403308 (indexed) against 520161 (baseline), a saving of 116853, which is less than the preprocessing spent. Total online work is 1452179 against 520161.",
          "Every completed generic/indexed pair has identical derived units and residual counters, so the difference between the two library arms is lookup cost alone; the 50 cases where the index is dearer are pure ternary cases with no binary clauses and two mixed cases with three binary clauses, by at most 68 events.",
          "No completed case has positive online savings over the baseline, so setup-plus-online cost 1453268 exceeds baseline for every batch size r>=1 by 1089+r*932018.",
        ],
      },
      citations: [],
      review: REVIEW,
      limitations: [
        "A finite constructed corpus with at most 12 declared variables and a toy residual solver; not a representative SAT workload and not a ranking of solvers.",
        "Counts are declared operational events under one implementation and cost model; not elapsed time, memory, or a proved bit-cost bound.",
        "The result concerns one known resolution schema and two ways of looking it up; it says nothing about general SAT or P versus NP.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:indexed-transfer-v1-total-cost-lesson", INDEXED_EDITION_KEY, INDEXED_EVIDENCE_KEY]),
    record("statement:implication-calibration-v1-coverage-and-decision-lesson", "statement", {
      proposition: "For the exact implication-calibration-v1 report bound below, all 120 cases are decided correctly by the DPLL and graph arms with no exhausted budgets, every decision certificate and every clue path passes the raw-clause checker, and the graph backbone equals the independent reference backbone on all 94 satisfiable cases. The frozen two-clause library finds 63 of the 173 reference backbone literals and none on the 24 long-explanation F_k cases; the implication graph finds all 173. The graph decision with its explicit certificate costs 124045 events against 100305 for DPLL and is cheaper on 18 of 120 cases; complete backbone extraction costs a further 167369 events. On this corpus the coverage gap of the local library is closed by the graph interface while the decision interface delta is negative, because the residual search of these small 2-CNFs is already near zero.",
      assumptions: [
        "The existing implication-calibration edition and observation evidence have been validated and independently reviewed.",
        "Inputs are CNFs of width at most two over at most 12 declared variables; the truth-table reference and the raw checker are outside all compared arms.",
        "Coverage and decision are separate comparisons; decision cost is compared only where both arms complete correctly, and the extra backbone work is never attributed to the decision.",
      ],
    }, ["context:research-method"]),
    record("assertion:implication-calibration-v1-coverage-and-decision-lesson", "assertion", {
      statement: "statement:implication-calibration-v1-coverage-and-decision-lesson",
      stance: "bounded-inference",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, [IMPLICATION_EDITION_KEY, IMPLICATION_EVIDENCE_KEY, "statement:implication-calibration-v1-coverage-and-decision-lesson"]),
    record("evidence:implication-calibration-v1-coverage-and-decision-lesson", "evidence", {
      assertion: "assertion:implication-calibration-v1-coverage-and-decision-lesson",
      kind: "bounded-result-interpretation",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Read the stored summary: 120 cases, 94 satisfiable and 26 unsatisfiable by reference; local, DPLL, and graph unknown counts are all zero; 120 certificates and 173 clue paths checked, all valid.",
          "Coverage on completed satisfiable cases: local 63 of 173 literals with the long-explanation family at 0 of 24 and the immediate-pattern controls at 42 of 42; graph 173 of 173 with every family fully covered. This is exactly the F_k gap deduced in statement:long-binary-backbone-explanations-v1.",
          "Decision cost: graph construction 40151, strongly connected components 67370, certificate 16524, total 124045; DPLL total 100305 with 429 search nodes. Graph better on 18, worse on 102, tied on 0. Backbone extraction adds 167369, more than the decision itself, consistent with its O(n(n+m)) all-literal search.",
          "Both interfaces decide these 2-SAT instances almost immediately, so no build and read cost can be repaid by residual savings here; the coverage result and the cost result are therefore independent and both stand.",
        ],
      },
      citations: [
        { title: "A linear-time algorithm for testing the truth of certain quantified Boolean formulas", authors: "Bengt Aspvall, Michael F. Plass, Robert Endre Tarjan", url: "https://doi.org/10.1016/0020-0190(79)90002-4", role: "Primary source for the strongly-connected-component decision used as the graph arm." },
        { title: "Local Backbones", authors: "Ronald de Haan, Iyad Kanj, Stefan Szeider", url: "https://arxiv.org/abs/1304.5479", role: "Bounded-clause explanations; the F_k family illustrates the local grammar's coverage limit." },
      ],
      review: REVIEW,
      limitations: [
        "A calibration of known 2-SAT reasoning on a finite corpus of at most 12 declared variables; no novelty, evolutionary advantage, or general SAT result.",
        "Counts are declared operational events; they do not establish the asymptotic bounds by measurement and are not elapsed time.",
        "Immediate-pattern controls overlap the training grammar and favor the local library by design.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:implication-calibration-v1-coverage-and-decision-lesson", IMPLICATION_EDITION_KEY, IMPLICATION_EVIDENCE_KEY]),
    record("inquiry:interface-delta-on-expensive-residuals-v3", "inquiry", {
      question: "Does any representation that a solver already builds, or any interface with proved polynomial build and read cost, achieve a positive interface delta on formulas whose residual search is expensive, such as 3-CNFs with binary substructure, under a protocol frozen before the corpus exists?",
      status: "open",
      designConstraint: "Charge build and read inside the arm, keep coverage and decision comparisons separate, use a residual solver whose search cost is large enough that a positive delta is possible, and preserve clue-transfer-v1, indexed-transfer-v1, and implication-calibration-v1 unchanged.",
      formalProofAccepted: false,
    }, ["assertion:interface-delta-v1", "assertion:indexed-transfer-v1-total-cost-lesson", "assertion:implication-calibration-v1-coverage-and-decision-lesson", "inquiry:clue-transfer-cost-and-coverage-v2"]),
  ];
}
