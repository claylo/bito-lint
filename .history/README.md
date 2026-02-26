# .history

This is where the reviews, audits, and course corrections live — out in the open.

I wrote the code. I wrote the template the code was generated from. I commissioned
external reviews to find the things I missed, and some of those things traced all the
way back to the template. That's how it works: you ship, you get feedback, you fix it,
you push fixes upstream so the next project doesn't inherit your mistakes.

None of this is curated to make me look good. The reviews found real bugs — a typo in
`--checks` that silently disabled all quality gates, an npm postinstall script with no
safety guards, a config doc that promised something the code couldn't deliver. I wrote
all of that code. The reviewers caught it. I fixed it.

**What's here:**

- **Code reviews** from external AI reviewers (Codex / GPT-5), dated and unedited
- **Security reviews** covering trust boundaries, supply chain, and resource exhaustion
- **Follow-up artifacts** as they accumulate

If you're reading this and thinking "why would someone publish their code review
findings?" — because the alternative is pretending the bugs didn't exist. I'd rather
show the process than polish the narrative. 

This is building in the open.
