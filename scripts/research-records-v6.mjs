import { canonicalJson, createKnowledgeGraphRecordV1 } from "@hraness/oh";

export const RESEARCH_PROFILE = "peqnp.research-ledger.v1";

// Reviewed v6 record: what counting the loading of algorithms and background
// knowledge can and cannot change, in answer to Album Shen's challenge.
function record(key, kind, value, dependencies) {
  return createKnowledgeGraphRecordV1({
    v: 1, key, kind, dependencies: [...dependencies].sort(),
    value: JSON.parse(canonicalJson({ profile: RESEARCH_PROFILE, ...value })),
  });
}

const REVIEW = {
  reviewerKind: "agent",
  status: "independently-reviewed-argument",
  scope: "The argument restates standard results and was independently reviewed in the project; this specification does not constitute a kernel receipt or human review.",
};

export function researchRecordsV6() {
  return [
    record("statement:loading-cost-and-advice-v1", "statement", {
      proposition: "Counting the loading of an algorithm, its premises, and its background knowledge as computation steps does not change any complexity class, because a fixed program is part of the machine and its size is a constant independent of the input; the constructive target's bound C(|F|+1)^k already covers every subroutine the program contains, and this laboratory already reports setup costs (rule mining, library compilation, interface construction) separately from online work. Knowledge permitted to depend on the input length is advice: a persistent store holding a precomputed answer or structure for every input of length n is a polynomial-size circuit family, the class P/poly, and by Karp and Lipton NP is contained in P/poly only if the polynomial hierarchy collapses to its second level. Charging more steps can only remove algorithms from P, so the accounting principle cannot assist a proof of P = NP, and because a constant load is asymptotically invisible it does not assist a proof of P != NP. What the principle leaves is amortization: persistent memory can lower the total cost of many related instances while leaving the worst case of one instance unchanged, and that quantity is measurable.",
      assumptions: [
        "Standard definitions: P as uniform deterministic polynomial time; P/poly as polynomial-size circuit families or polynomial advice per input length.",
        "The laboratory's declared event model, in which setup and online work are separately charged.",
      ],
    }, ["context:research-method", "assertion:interface-delta-v1"]),
    record("assertion:loading-cost-and-advice-v1", "assertion", {
      statement: "statement:loading-cost-and-advice-v1",
      stance: "informal-argument-reviewed",
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["statement:loading-cost-and-advice-v1"]),
    record("evidence:loading-cost-and-advice-v1", "evidence", {
      assertion: "assertion:loading-cost-and-advice-v1",
      kind: "reviewed-argument",
      argument: {
        format: "ordered-prose-steps",
        steps: [
          "Album Shen proposed that loading an algorithm, its premises, and background knowledge should count as timesteps, since a library import condenses code, and that persistent external memory is where a shortcut may lie; Ben Guo asked whether the idea has experimental content.",
          "A Turing machine has no imports: its transition table is its entire library, so the standard polynomial bound already charges every subroutine a program contains; loading a fixed library costs a constant, which changes C and never k.",
          "If the loaded knowledge may depend on the input length it is advice; the knowledge-and-complexity table already places it in P/poly, and Karp and Lipton (1980) show NP in P/poly would collapse the polynomial hierarchy, the standard reason it is believed false.",
          "Adding charges shrinks P; it cannot move a problem into P, so the principle cannot support P = NP, and a constant charge is asymptotically invisible, so it does not support P != NP.",
          "The surviving content is amortization across related instances, which the amortized-interface protocol measures as the query count at which a once-built fragment interface repays its construction.",
        ],
      },
      citations: [
        { title: "Some connections between nonuniform and uniform complexity classes", authors: "Richard M. Karp, Richard J. Lipton", url: "https://doi.org/10.1145/800141.804678", role: "NP in P/poly implies the polynomial hierarchy collapses to its second level." },
        { title: "Computational Complexity: A Modern Approach, chapter 6", authors: "Sanjeev Arora, Boaz Barak", url: "https://theory.cs.princeton.edu/complexity/book.pdf", role: "Advice, circuit families, and the P/poly definitions." },
        { title: "A Knowledge Compilation Map", authors: "Adnan Darwiche, Pierre Marquis", url: "https://www.cs.cmu.edu/afs/cs.cmu.edu/project/jair/pub/volume17/darwiche02a.pdf", role: "Compile once, query many; compiled forms can be exponentially large." },
      ],
      review: REVIEW,
      limitations: [
        "The argument settles what the accounting principle can change, not whether any particular amortized interface repays itself; that is an experiment.",
        "Credits describe conceptual contributions; Album Shen's proposal is the challenge, the analysis is the project's.",
      ],
      formalProofAccepted: false,
      kernelReceipt: null,
    }, ["assertion:loading-cost-and-advice-v1"]),
  ];
}
