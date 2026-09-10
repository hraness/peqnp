import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";

// Reviewed v4 records: the complete 2-CNF backbone is not known to be readable
// in linear time, an earlier report sentence claiming otherwise is withdrawn,
// and the v4 inquiry's premise is replaced. These records add no experiment.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

const REVIEW = {
  reviewerKind: "agent",
  status: "independently-reviewed-argument",
  scope: "Three independent reviewers attempted to refute the claim, two of them by brute-force checking reductions against the repository's own extractor and checker; this specification does not constitute a kernel receipt or human review.",
};

export function researchRecordsV4() {
  return [
    record("statement:two-cnf-backbone-not-known-linear-v1", "statement", {
      proposition: "No algorithm is known that computes the complete set of forced literals of a satisfiable 2-CNF with n variables and m clauses in O(n+m) time. For a directed graph G, the 2-CNF Gamma_{G,3} with variables v^1, v^2, v^3 per vertex and clauses u^1 -> v^2, u^2 -> v^3, u^3 -> not v^1 per edge (u,v) forces v^1 false exactly when v lies on a triangle of G (Buss, Kullmann, and Vassilevska Williams, Theorem 11). Hence an O(m^c) complete-backbone algorithm gives an O(m^c) algorithm listing every vertex on a triangle, and a near-linear one would contradict the conjectured m^{4/3-o(1)} triangle-detection lower bound and, through k-cycle generalizations, give O((2-delta)^n) Max-k-SAT algorithms. Three project sentences claiming that the component condensation answers every forced-literal query in O(n+m_2) total, or in one linear pass, were unsupported and are withdrawn: two in earlier editions of the fragment-interface report (its lesson on extraction cost and its closing section on the next experiment) and one in the knowledge framework's strategy list.",
      assumptions: [
        "Standard definitions: a literal l is forced by a satisfiable 2-CNF exactly when the implication graph has a path from not l to l (Aspvall, Plass, and Tarjan; Buss et al. Proposition 1).",
        "Word-RAM model with O(log m)-bit words, as in the cited hardness statements.",
        "The withdrawn sentences are project errors, not claims of the cited paper.",
      ],
    }, ["context:research-method", "assertion:fragment-interface-v1-first-positive-delta-lesson"]),
    record("assertion:two-cnf-backbone-not-known-linear-v1", "assertion", {
      statement: "statement:two-cnf-backbone-not-known-linear-v1",
      stance: "informal-argument-reviewed",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["statement:two-cnf-backbone-not-known-linear-v1"]),
    record("evidence:two-cnf-backbone-not-known-linear-v1", "evidence", {
      assertion: "assertion:two-cnf-backbone-not-known-linear-v1",
      kind: "reviewed-literature-and-argument",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "The withdrawn sentences assert a batch of 2n reachability queries on the condensation in linear total time. No linear-time method for that batch of matched-pair reachability queries is known; nothing in the repository or in Aspvall, Plass, and Tarjan supports a linear total.",
          "Buss, Kullmann, and Vassilevska Williams define Gamma_{G,3} and prove Theorem 11: G has a triangle involving v iff Gamma_{G,3} forces v^1 false. Their stated consequence is that an O(m^c) backbone algorithm gives an O(m^c) algorithm for all vertices on triangles, and even a single failed literal in O(m^c) gives O(m^c) triangle detection.",
          "The best known triangle detection runs in O(m^{1.407}) with fast matrix multiplication and O(m^{4/3}) even if omega = 2; m^{4/3-o(1)} is conjectured necessary. Their Theorem 12 generalizes to k-cycles in directed graphs, so under the k-Cycle Hypothesis, for any epsilon > 0, no O(m^{2-epsilon}) complete-backbone algorithm exists.",
          "Two reviewers independently rebuilt a triangle reduction after finding that a one-layer sketch collapses (on the triangle-free 4-cycle it forces every s_i), and verified the three-layer construction by exhaustive enumeration on small graphs and against the repository's own extractor, truth-table reference, and raw-clause checker; the witness paths have exactly three edges and are accepted by the checker.",
          "Heule, Jarvisalo, and Biere state that fixpoint computation of failed literals on the binary implication graph is conjectured to be at least quadratic; Brafman's 2-SIMPLIFY reads implied units from a transitive closure; del Val's literal-probing variant is O(nm). The literature contains no linear complete-backbone algorithm.",
          "Consequently the extraction-cost protocol states the same O(n(n+m_2)) worst case for per-literal and settled extraction and tests constant-factor savings only.",
        ],
      },
      citations: [
        { title: "Dual Depth First Search for Binary Clause Reasoning", authors: "Sam Buss, Oliver Kullmann, Virginia Vassilevska Williams", url: "https://mathweb.ucsd.edu/~sbuss/ResearchWeb/DualDFS/DualDFS_SAT.pdf", role: "Theorem 11 and 12 reductions from triangle and k-cycle detection to 2-CNF backbone; revision of 6 May 2024." },
        { title: "Efficient CNF Simplification Based on Binary Implication Graphs", authors: "Marijn Heule, Matti Jarvisalo, Armin Biere", url: "https://www.cs.utexas.edu/~marijn/publications/unhiding.pdf", role: "Failed literals on the binary implication graph and the at-least-quadratic conjecture for their fixpoint." },
        { title: "A linear-time algorithm for testing the truth of certain quantified Boolean formulas", authors: "Bengt Aspvall, Michael F. Plass, Robert Endre Tarjan", url: "https://doi.org/10.1016/0020-0190(79)90002-4", role: "Linear-time decision; decides one forced-literal query per linear pass, not all of them." },
        { title: "Finding and counting given length cycles", authors: "Noga Alon, Raphael Yuster, Uri Zwick", url: "http://www.math.tau.ac.il/~nogaa/PDFS/ayz4.pdf", role: "Best known sparse-graph triangle detection bound." },
      ],
      review: REVIEW,
      limitations: [
        "This is a conditional hardness statement from the literature and an elementary reduction, not a proof that linear extraction is impossible.",
        "The withdrawal corrects project prose; the fragment-interface-v1 measurements and artifact are unaffected.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:two-cnf-backbone-not-known-linear-v1"]),
    record("inquiry:constant-factor-extraction-and-solver-owned-interfaces-v5", "inquiry", {
      question: "With no linear read of all forced literals available, does a constant-factor cheaper complete extraction, one stamped array with inheritance and settling, make the fragment interface's delta positive on mixed 2/3-CNFs whose fragment forces literals; and does an interface a solver already builds expose the same literals at marginal read cost?",
      status: "open",
      supersedes: "inquiry:linear-extraction-and-solver-owned-interfaces-v4",
      designConstraint: "The v4 inquiry assumed a linear pass over the condensation; that premise is withdrawn by statement:two-cnf-backbone-not-known-linear-v1. The extraction-cost protocol is frozen before its corpus exists, keeps the fragment-interface-v1 corpus as a matched secondary comparison, and requires identical processed formulas and residual counters between the two extraction arms.",
      formalProofAccepted: false,
    }, ["assertion:two-cnf-backbone-not-known-linear-v1", "inquiry:linear-extraction-and-solver-owned-interfaces-v4"]),
  ];
}
