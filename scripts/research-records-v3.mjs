import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";

// Reviewed v3 argument: the full-text reading of Levin's pointer framing.
// It depends on the v2 interface-delta definition and adds no experiment.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

const REVIEW = {
  reviewerKind: "agent",
  status: "independently-reviewed-argument",
  scope: "The reading and the elementary complexity-theoretic argument were independently reviewed in the project; this specification does not constitute a kernel receipt or human review.",
};

export const FRAGMENT_REPORT_SHA256 = "ee4d85dbfd4ee3c981f401d482c61bc05fa68f035de6d54b853b03c30d3a6375";
export const FRAGMENT_EDITION_KEY = "edition:fragment-interface-" + FRAGMENT_REPORT_SHA256;
export const FRAGMENT_EVIDENCE_KEY = "evidence:fragment-interface-f046a86f9075375e79d620c9bef82597cf2de5f2fa780b42f41a219384af112c";

export function researchRecordsV3() {
  const fragmentEvidenceKey = FRAGMENT_EVIDENCE_KEY;
  return [
    record("statement:pointer-certificate-asymmetry-v1", "statement", {
      proposition: "Levin's distinction between a pointer and a compression, that a short seed unfolds into a rich pattern while no procedure recovers the seed from the pattern, is the certificate asymmetry that defines NP: a certificate is a pointer whose referent is checkable in polynomial time, and the open question is whether such a pointer can always be found from its referent within a polynomial bound. His free lunches, the third angle of a triangle from two known angles and every logic gate from one transistor, are the cases in which the inverse map is also cheap. Poincare's examiner, the prior examination that removes sterile combinations before invention, is read as a pre-filter; if such a filter runs in polynomial time and always leaves polynomially many candidates for a polynomial-time consumer, the composition lemma places the language in P. P = NP is therefore equivalent to the universal form of the free-lunch claim, and nothing in the article decides it.",
      assumptions: [
        "Standard definitions: NP as polynomial-time verifiable certificates; P as deterministic polynomial time; the uniform worst-case target of the project.",
        "The reading concerns only the operational content of the article; its metaphysics of non-physical or agential patterns is neither adopted nor evaluated.",
        "Following a pointer is identified with verification and finding one with search; a pointer is not required to be unique.",
      ],
    }, ["context:research-method", "assertion:interface-delta-v1"]),
    record("assertion:pointer-certificate-asymmetry-v1", "assertion", {
      statement: "statement:pointer-certificate-asymmetry-v1",
      stance: "informal-argument-reviewed",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["statement:pointer-certificate-asymmetry-v1"]),
    record("evidence:pointer-certificate-asymmetry-v1", "evidence", {
      assertion: "assertion:pointer-certificate-asymmetry-v1",
      kind: "reviewed-definition-and-argument",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "In the article's section on the origin of patterns, a short complex function such as z^3+7 points at a Halley fractal that a simple algorithm reveals; a footnote states that this is not compression because no algorithm takes the image and returns a seed, and that both are discovered while only the forward direction can be followed.",
          "A language L is in NP exactly when there is a polynomial-time relation R and polynomial p such that x is in L iff some certificate c with |c| <= p(|x|) satisfies R(x,c). The certificate c is a pointer whose referent x is checked by following R; finding c from x is the search problem.",
          "For the triangle and the gate the inverse map is polynomial: the third angle is a subtraction, and a gate's truth table is evaluated in constant time per row. These are free lunches because finding the pointer costs no more than following it.",
          "The article's Poincare epigraph describes invention as choice among candidates that survived a prior examination. A polynomial-time examiner leaving polynomially many candidates for a polynomial-time consumer decides the language in polynomial time by the composition lemma recorded in the knowledge framework.",
          "Hence the operational claim of the article, that following pointers is cheap and interfaces must be investigated, coincides with the setting of P versus NP; the universal claim that every efficiently checkable pointer is efficiently findable is P = NP itself and is not asserted by the article or assumed by this project.",
          "The reading was checked against the accepted preprint v4 (OSF, June 2026), whose title matches the published article; the publisher's page blocks automated retrieval.",
        ],
      },
      citations: [
        { title: "Ingressing Minds: Causal, Non-Physical Patterns In-Form Natural, Synthetic, and Hybrid Embodiments", authors: "Michael Levin", url: "https://doi.org/10.3390/philosophies11050161", role: "Published article; the pointer footnote and Poincare epigraph are in its section on the origin of pattern memories." },
        { title: "Ingressing Minds, preprint v4", authors: "Michael Levin", url: "https://doi.org/10.31234/osf.io/5g2xj_v4", role: "Full text read for this record; accepted version with the published title." },
        { title: "The P versus NP problem", authors: "Stephen Cook", url: "https://www.claymath.org/wp-content/uploads/2022/06/pvsnp.pdf", role: "Standard definitions of certificates, NP, and the problem statement." },
      ],
      review: REVIEW,
      limitations: [
        "The correspondence is a reading of the article's operational vocabulary into standard definitions; it is not a theorem about the article and not evidence for either resolution of P versus NP.",
        "Poincare's account is psychological; its use here is as the informal source of the pre-filter idea, not as a claim about human cognition.",
        "The free-lunch examples are instances of cheap inversion; they establish nothing about the inversion cost of arbitrary NP relations.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:pointer-certificate-asymmetry-v1", "assertion:interface-delta-v1"]),
    record("statement:fragment-interface-v1-first-positive-delta-lesson", "statement", {
      proposition: "For the exact fragment-interface-v1 report bound below, both arms decide all 162 mixed 2/3-CNF cases correctly with no exhausted budget, every fragment certificate and clue path passes the raw-clause checker, and derived units are contained in the reference backbone on every satisfiable case. The fragment arm's total work of 1245876 events exceeds the 961854 baseline on 140 cases and is lower on 22. Twenty-one of the 22 are cases whose binary fragment is unsatisfiable, so the fragment certificate decides the whole formula with zero residual work; the remaining one is a mixed case with n binary clauses where three forced units cut the residual from 11131 to 2905 events. On the 21 searched cases with 2n binary clauses the fragment's 142 forced units reduce residual work from 112990 to 60841 events and search nodes from 75 to 23, yet the arm loses every one because its phases cost 116279 events, of which extraction is the largest part. On the 36 ternary-only cases the residual equals the baseline exactly and the phases are pure overhead. The interface delta is therefore positive on this corpus exactly when the interface's read is the decision, and negative whenever clues must be extracted by per-literal search.",
      assumptions: [
        "The existing fragment-interface edition and observation evidence have been validated and independently reviewed.",
        "Inputs are CNFs of width at most three over at most 12 declared variables; the truth-table reference and the raw checker are outside both arms.",
        "Both arms share one deterministic DPLL and one event model; the fragment arm's six phases are charged on one meter before its residual.",
      ],
    }, ["context:research-method", "assertion:interface-delta-v1"]),
    record("assertion:fragment-interface-v1-first-positive-delta-lesson", "assertion", {
      statement: "statement:fragment-interface-v1-first-positive-delta-lesson",
      stance: "bounded-inference",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, [FRAGMENT_EDITION_KEY, fragmentEvidenceKey, "statement:fragment-interface-v1-first-positive-delta-lesson"]),
    record("evidence:fragment-interface-v1-first-positive-delta-lesson", "evidence", {
      assertion: "assertion:fragment-interface-v1-first-positive-delta-lesson",
      kind: "bounded-result-interpretation",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Read the stored summary: 162 cases, 56 satisfiable and 106 unsatisfiable by reference; both arms complete 162 with 0 unknown; fragment_vs_baseline both_complete 162, left_better 22, tied 0, left_worse 140; 162 certificates and 205 clue paths checked, all valid.",
          "Enumerate the 22 winning observations: 15 random-mixed cases with 2n binary clauses and all 6 contradictory-fragment controls carry an opposite-paths fragment certificate with residual_work_units 0; the one remaining winner, random-n12-t3-b12-s3001, has 3 derived units, residual 2905, and phases 3912 against a baseline of 11131.",
          "Sum the random-mixed subtotal at b=2n over the 21 cases with positive residual: baseline 112990 with 75 search nodes, fragment residual 60841 with 23 nodes, phases 116279, 142 units, 0 wins. Phase totals over the corpus: construction 79128, components 91170, certificate 16422, extraction 256126, append 52375; extraction is 256126 of 495221.",
          "On all 36 random-ternary-only observations the fragment residual work equals the baseline total and the derived-unit list is empty; the phase total of 75720 is therefore overhead of an empty interface.",
          "Hence the delta, as defined in statement:interface-delta-v1, is positive on this corpus exactly on the cases where the fragment certificate makes the residual zero, and the extraction phase is the cost that prevents forced literals from repaying their read.",
        ],
      },
      citations: [
        { title: "A linear-time algorithm for testing the truth of certain quantified Boolean formulas", authors: "Bengt Aspvall, Michael F. Plass, Robert Endre Tarjan", url: "https://doi.org/10.1016/0020-0190(79)90002-4", role: "Primary source for the fragment decision; its component condensation also answers every forced-literal query in linear time, the next protocol's candidate." },
        { title: "Backdoors To Typical Case Complexity", authors: "Ryan Williams, Carla P. Gomes, Bart Selman", url: "https://www.cs.cornell.edu/selman/papers/pdf/03.ijcai.backdoors.pdf", role: "Tractable sub-structure exploited by a subsolver; the binary fragment is read rather than searched." },
      ],
      review: REVIEW,
      limitations: [
        "A finite constructed corpus of at most 12 variables and width three with a toy residual solver; the contradictory controls were built to be decided by the certificate.",
        "Counts are declared operational events; not elapsed time, memory, or a proved bit-cost bound, and the stated phase bounds are design arguments.",
        "The result concerns one interface on one corpus; it says nothing about general SAT or P versus NP.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:fragment-interface-v1-first-positive-delta-lesson", FRAGMENT_EDITION_KEY, fragmentEvidenceKey]),
    record("inquiry:linear-extraction-and-solver-owned-interfaces-v4", "inquiry", {
      question: "Does reading every forced literal of the binary fragment from the strongly-connected-component condensation in one linear pass, instead of one depth-first search per literal, make the fragment interface's delta positive on mixed 2/3-CNFs whose fragment forces literals; and does an interface a solver already builds expose the same literals at marginal read cost?",
      status: "open",
      designConstraint: "Freeze the protocol before the corpus exists, keep the fragment-interface-v1 corpus generator available for a matched comparison, charge build and read inside the arm, and preserve all four earlier artifacts unchanged.",
      formalProofAccepted: false,
    }, ["assertion:fragment-interface-v1-first-positive-delta-lesson", "assertion:pointer-certificate-asymmetry-v1", "inquiry:interface-delta-on-expensive-residuals-v3"]),
  ];
}
