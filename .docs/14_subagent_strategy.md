# Sub-agent strategy

The user wants sub-agents used deliberately, not spammed.

## Required sub-agent passes

### Pass 1 — architecture sanity review

When:
- after initial workspace/module plan exists
- before heavy implementation begins

Ask for:
- boundary sanity
- concurrency/state ownership sanity
- likely cross-platform pain points

### Pass 2 — media/export/licensing sanity review

When:
- provider abstraction and export families are drafted
- before export behavior is locked

Ask for:
- runtime/provider separation sanity
- export family sanity
- likely compliance pitfalls
- Adobe compatibility sanity

### Pass 3 — final QA/docs sanity review

When:
- app is mostly complete
- manuals and report are drafted

Ask for:
- doc/code mismatch detection
- likely missing tests
- suspicious UX inconsistencies
- incomplete implementation-report areas

## Rules

- keep concurrency low (1–2)
- wait for results
- terminate clearly orphaned unrelated sub-agents
- record findings in the implementation report
