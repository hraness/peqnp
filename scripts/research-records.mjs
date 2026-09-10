import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";
export const RESEARCH_REPORT_SHA256 = "dd3293c5466fb64eb7b0406e61afb42f3ae52d00ea86073ffa83a0dd184fd85f";
export const RESEARCH_TRANSFER_EDITION_KEY = "edition:clue-transfer-dd3293c5466fb64eb7b0406e61afb42f3ae52d00ea86073ffa83a0dd184fd85f";
export const RESEARCH_TRANSFER_EVIDENCE_KEY = "evidence:clue-transfer-e93f4f15d185ee57138bc83edce1895fa6c0de1ea267e24e6f3167b0d082f0b9";

// These reviewed v1 arguments intentionally retain their original experiment dependency.
// Revise the record keys when an argument or its evidence changes; do not retarget history.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

export function researchRecords() {
  return [
    record("statement:signed-injective-entailment-v1", "statement", {
      "proposition": "For every finite clause set P and literal l with P entailing l, every complement-preserving signed injective variable renaming sigma, and every finite CNF F containing sigma(P), F entails sigma(l). Adding sigma(l) as a unit preserves exactly the satisfying assignments of F.",
      "assumptions": [
        "Classical Boolean semantics; P entails l on every assignment.",
        "Sigma is defined on every variable occurring in P or l. Distinct source variables map to literals on distinct target variables; sigma(not v)=not sigma(v).",
        "Every renamed premise clause is present in F; syntactic similarity alone is insufficient."
      ]
    }, ["context:research-method"]),
    record("assertion:signed-injective-entailment-v1", "assertion", {
      "statement": "statement:signed-injective-entailment-v1",
      "stance": "informal-argument-reviewed",
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["statement:signed-injective-entailment-v1"]),
    record("evidence:signed-injective-entailment-v1", "evidence", {
      "assertion": "assertion:signed-injective-entailment-v1",
      "kind": "reviewed-mathematical-argument",
      "argument": {
        "format": "ordered-prose-steps",
        "steps": [
          "Let A be any satisfying assignment of F. Then A satisfies all clauses of sigma(P).",
          "Pull A back along sigma: assign each source variable the truth value of its signed target literal. Complement preservation makes evaluation of every renamed clause agree.",
          "This source assignment satisfies P and therefore l. Hence A satisfies sigma(l). Since A was arbitrary, F entails sigma(l), and adjoining it changes no satisfying assignment.",
          "The mined schema (a or b) and (a or not b) entails a: assuming a=false requires b=true and b=false simultaneously. This independently establishes the premise entailment for its signed instances."
        ]
      },
      "citations": [],
      "review": {
        "reviewerKind": "agent",
        "status": "independently-reviewed-argument",
        "scope": "The mathematical argument and measured-result interpretation were independently reviewed in the project; this specification does not constitute a kernel receipt or human review."
      },
      "limitations": [
        "General informal proof; not inferred solely from the six training examples.",
        "Entailment remains valid for UNSAT F, but describing its literals as backbones of a nonempty solution set would be inappropriate.",
        "No per-match proof trace or kernel proof is supplied by experiment v1."
      ],
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["assertion:signed-injective-entailment-v1"]),
    record("statement:clue-transfer-v1-total-cost-lesson", "statement", {
      "proposition": "For the exact clue-transfer-v1 report bound below, both arms complete all 122 cases correctly; the frozen library derives 127 units and reduces total search nodes from 738 to 620, while total online declared work rises from 424997 to 1676645 and is greater on every case. Repeated reruns of this unchanged workload and algorithms cannot amortize the 1000-event acquisition cost.",
      "assumptions": [
        "The existing report edition and observation evidence have been validated and independently reviewed.",
        "Same deterministic DPLL and event model in both arms; one root preprocessor pass; each arm has one million total events including preprocessing.",
        "The repetition conclusion concerns unchanged per-query algorithms with a reused frozen library; it introduces no new answer cache or altered workload."
      ]
    }, ["context:research-method"]),
    record("assertion:clue-transfer-v1-total-cost-lesson", "assertion", {
      "statement": "statement:clue-transfer-v1-total-cost-lesson",
      "stance": "bounded-inference",
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["edition:clue-transfer-dd3293c5466fb64eb7b0406e61afb42f3ae52d00ea86073ffa83a0dd184fd85f", "evidence:clue-transfer-e93f4f15d185ee57138bc83edce1895fa6c0de1ea267e24e6f3167b0d082f0b9", "statement:clue-transfer-v1-total-cost-lesson"]),
    record("evidence:clue-transfer-v1-total-cost-lesson", "evidence", {
      "assertion": "assertion:clue-transfer-v1-total-cost-lesson",
      "kind": "bounded-result-interpretation",
      "argument": {
        "format": "ordered-prose-steps",
        "steps": [
          "Read the exact stored report summary and each observation. Both complete counts are 122, unknown counts are zero, transfer_better=0, tied=0, transfer_worse=122.",
          "The online event difference is 1676645-424997=1251648. Search nodes fall by 738-620=118, so reduced search does not imply reduced total declared work.",
          "For an integer r>=1 repeated full workloads, the transfer cost including one acquisition exceeds baseline by 1000+r*1251648, which is strictly positive."
        ]
      },
      "citations": [],
      "review": {
        "reviewerKind": "agent",
        "status": "independently-reviewed-argument",
        "scope": "The mathematical argument and measured-result interpretation were independently reviewed in the project; this specification does not constitute a kernel receipt or human review."
      },
      "limitations": [
        "A finite nonrepresentative corpus with at most 12 declared variables; some controls use only three active variables.",
        "Some density variants share pseudorandom prefixes. No statistical population guarantee is inferred.",
        "Counts are declared operational events, excluding allocator internals and other stated overhead; they are not elapsed time or a proved bit-cost bound.",
        "The result concerns this implementation and workload, not all preprocessing or P versus NP."
      ],
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["assertion:clue-transfer-v1-total-cost-lesson", "edition:clue-transfer-dd3293c5466fb64eb7b0406e61afb42f3ae52d00ea86073ffa83a0dd184fd85f", "evidence:clue-transfer-e93f4f15d185ee57138bc83edce1895fa6c0de1ea267e24e6f3167b0d082f0b9"]),
    record("statement:long-binary-backbone-explanations-v1", "statement", {
      "proposition": "For every integer k>=2 and distinct variables x,y_1,...,y_k, define F_k=(x or y_1) and AND_{i=1}^{k-1}(not y_i or y_{i+1}) and (not y_k or x). F_k is satisfiable, has k+1 clauses, entails x, and no proper clause subset entails x. No two-clause subformula entails any unit, so the frozen binary-pair-to-unit library cannot initiate an inference on F_k.",
      "assumptions": [
        "All variables are distinct; clauses and semantics are exactly as displayed.",
        "The library only concludes an entailed unit from a syntactically present pair of clauses; derived binary clauses or failed-literal closure are not available."
      ]
    }, ["context:research-method"]),
    record("assertion:long-binary-backbone-explanations-v1", "assertion", {
      "statement": "statement:long-binary-backbone-explanations-v1",
      "stance": "informal-argument-reviewed",
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["statement:long-binary-backbone-explanations-v1"]),
    record("evidence:long-binary-backbone-explanations-v1", "evidence", {
      "assertion": "assertion:long-binary-backbone-explanations-v1",
      "kind": "reviewed-mathematical-argument",
      "argument": {
        "format": "ordered-prose-steps",
        "steps": [
          "There are 2+(k-1)=k+1 clauses. Setting x=true and all y_i=false satisfies F_k. Setting x=true and all y_i=true also satisfies F_k; thus no y_i or not y_i is entailed.",
          "If x=false, the first clause forces y_1=true, the middle links force every y_i=true, and the last clause is false. Therefore F_k entails x.",
          "If the first clause is deleted, x=false with every y_i=false satisfies the remainder. If the last is deleted, use x=false and every y_i=true.",
          "If the internal link (not y_i or y_{i+1}) is deleted, use x=false, y_1 through y_i=true, and the remaining y values=false. Every remaining clause holds.",
          "Every proper subset omits a clause, so one of those witnesses satisfies it with x=false. Any two-clause subset is proper for k>=2 and cannot entail x. It cannot entail either sign of any y_i because F_k itself permits both signs, nor not x because F_k has models with x=true. Hence no sound unit conclusion is available from any pair.",
          "The implication path not x -> y_1 -> ... -> y_k -> x exposes the longer explanation. This is a limitation of the small explanation grammar; F_k belongs to polynomial-time decidable 2-SAT."
        ]
      },
      "citations": [
        {
          "title": "Local Backbones",
          "authors": "Ronald de Haan, Iyad Kanj, Stefan Szeider",
          "url": "https://arxiv.org/abs/1304.5479",
          "role": "Primary source for local-backbone terminology; F_k argument here is a project deduction."
        },
        {
          "title": "A linear-time algorithm for testing the truth of certain quantified Boolean formulas",
          "authors": "Bengt Aspvall, Michael F. Plass, Robert Endre Tarjan",
          "url": "https://doi.org/10.1016/0020-0190(79)90002-4",
          "role": "Primary source for implication-graph and strongly-connected-component 2-SAT baseline."
        }
      ],
      "review": {
        "reviewerKind": "agent",
        "status": "independently-reviewed-argument",
        "scope": "The mathematical argument and measured-result interpretation were independently reviewed in the project; this specification does not constitute a kernel receipt or human review."
      },
      "limitations": [
        "This family is outside the frozen 122-case experiment and was not added post hoc.",
        "The proof is informal and does not assert hardness of 2-SAT or impossibility of efficient broader clue extraction."
      ],
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["assertion:long-binary-backbone-explanations-v1"]),
    record("statement:uniform-logarithmic-strong-backdoors-v1", "statement", {
      "proposition": "Fix a sound polynomial-time partial SAT subsolver S and a promised CNF family C. If a uniform polynomial-time algorithm finds, for every F in C of encoded length N>=2, a variable set B of size at most c*log_2(N), for fixed c, such that S decides every restriction F|alpha for alpha assigning B, then SAT on C has a uniform polynomial-time decision algorithm.",
      "assumptions": [
        "S is one fixed deterministic sound subsolver: any SAT/UNSAT answer is correct, and it finishes every run in polynomial bit time, possibly returning unknown outside its solved class.",
        "For the returned B, all 2^|B| restrictions are correctly decided by S; this is the strong-backdoor condition.",
        "The algorithm finding B is uniform, correct throughout C, and polynomial in N. Restrictions have polynomial representation/construction cost."
      ]
    }, ["context:research-method"]),
    record("assertion:uniform-logarithmic-strong-backdoors-v1", "assertion", {
      "statement": "statement:uniform-logarithmic-strong-backdoors-v1",
      "stance": "informal-argument-reviewed",
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["statement:uniform-logarithmic-strong-backdoors-v1"]),
    record("evidence:uniform-logarithmic-strong-backdoors-v1", "evidence", {
      "assertion": "assertion:uniform-logarithmic-strong-backdoors-v1",
      "kind": "reviewed-mathematical-argument",
      "argument": {
        "format": "ordered-prose-steps",
        "steps": [
          "Compute B with the assumed uniform algorithm.",
          "Enumerate each Boolean assignment alpha to B, construct F|alpha, and run S. Return SAT if any restriction is SAT; otherwise return UNSAT. Every satisfying assignment of F extends one such alpha, so this decision is correct.",
          "There are at most 2^(c*log_2(N))=N^c restrictions. Multiplying this by polynomial restriction and S costs, then adding polynomial discovery, gives a fixed polynomial bound in N."
        ]
      },
      "citations": [
        {
          "title": "Backdoors To Typical Case Complexity",
          "authors": "Ryan Williams, Carla P. Gomes, Bart Selman",
          "url": "https://www.cs.cornell.edu/selman/papers/pdf/03.ijcai.backdoors.pdf",
          "role": "Primary source for subsolver-relative backdoor definitions and the distinction between exploiting and finding them; the logarithmic-size consequence follows by the displayed enumeration argument."
        }
      ],
      "review": {
        "reviewerKind": "agent",
        "status": "independently-reviewed-argument",
        "scope": "The mathematical argument and measured-result interpretation were independently reviewed in the project; this specification does not constitute a kernel receipt or human review."
      },
      "limitations": [
        "Backbones fix values; backdoors select variables relative to a subsolver. The concepts are different.",
        "Neither existence nor efficient discovery of such small strong backdoors is established here for arbitrary SAT.",
        "This conditional theorem supplies no missing discovery algorithm and no P=NP result."
      ],
      "formalProofAccepted": false,
      "kernelReceipt": null
    }, ["assertion:uniform-logarithmic-strong-backdoors-v1"]),
    record("inquiry:clue-transfer-cost-and-coverage-v2", "inquiry", {
      "question": "Can an indexed matcher lower the total cost of the same sound rule library on fresh cases, and can compositional binary inference expose longer clues when compared with an established polynomial 2-SAT baseline?",
      "status": "open",
      "designConstraint": "Specify separate cost and coverage interventions and a fresh evaluation protocol before observing results; preserve clue-transfer-v1.",
      "formalProofAccepted": false
    }, ["assertion:clue-transfer-v1-total-cost-lesson", "assertion:long-binary-backbone-explanations-v1", "assertion:uniform-logarithmic-strong-backdoors-v1"]),
  ];
}
