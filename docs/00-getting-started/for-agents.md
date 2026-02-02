# Quick Start for AI Agents

**Version**: 0.1.0 | **Last Updated**: 2026-02-02

## You Are Here

This is your quick reference for working in the Thalamus repository. For complete guidelines, see [.claude/agent-guidelines.md](../../.claude/agent-guidelines.md).

## Critical First Steps

1. **Read [BOUNDARIES.md](../../BOUNDARIES.md)** - NON-NEGOTIABLE, read this first
2. **Read [.claude/agent-guidelines.md](../../.claude/agent-guidelines.md)** - Your operational manual
3. Check current phase below
4. Review this checklist before ANY work

## Current Phase: Foundation (Phase 0)

### Status Dashboard

```
Phase: 0 (Foundation)
Status: Documentation and conceptual architecture
Code: None (intentionally)
Your Role: Documentation and conceptual work only
```

### What You CAN Do ✅

- Improve documentation clarity
- Identify gaps in coverage
- Propose architectural refinements
- Create/enhance examples
- Fix typos, formatting, links
- Enhance navigation
- Document design decisions
- Clarify terminology

### What You CANNOT Do ❌

- Write implementation code
- Choose technologies/frameworks
- Create build configurations
- Write tests (no code to test)
- Make technology decisions
- Create CI/CD pipelines

## The Mandatory Workflow

### Before ANY Contribution

```
┌─────────────────────────────────────┐
│ 1. Read BOUNDARIES.md (if not done) │
├─────────────────────────────────────┤
│ 2. Apply Five-Question Framework    │
├─────────────────────────────────────┤
│ 3. Check phase restrictions         │
├─────────────────────────────────────┤
│ 4. Determine decision level         │
├─────────────────────────────────────┤
│ 5. Document reasoning               │
├─────────────────────────────────────┤
│ 6. Execute or propose               │
└─────────────────────────────────────┘
```

### Five-Question Framework (from BOUNDARIES.md)

Run this check on EVERY feature or change:

```
1. Signal Question: Is this about signal routing/normalization?
   [ ] YES → Likely belongs  [ ] NO → Belongs elsewhere

2. Decision Question: Does this make business decisions?
   [ ] YES → Does NOT belong  [ ] NO → May belong

3. Domain Question: Is this specific to one product?
   [ ] YES → Does NOT belong  [ ] NO → May belong

4. State Question: Does this require long-term persistent state?
   [ ] YES → Does NOT belong  [ ] NO → May belong

5. Reusability Question: Would every RBX product need this?
   [ ] YES → Likely belongs  [ ] NO → Product-specific
```

**All five must pass for feature to belong in Thalamus.**

## Decision Levels Quick Reference

### Autonomous (Just Do It)
- Typo fixes
- Formatting improvements
- Link fixes
- Adding examples to existing sections
- Grammar corrections

**Action**: Make change, document in commit message

### Reviewed (Propose First)
- New conceptual sections
- Architectural refinements
- Boundary clarifications
- Navigation restructuring
- Process changes

**Action**: Document in design-decisions.md, request review, wait for approval

### Governed (Human Decision)
- Technology choices
- Phase transitions
- Major architectural changes
- Boundary definition changes
- License decisions

**Action**: Present options with analysis, wait for human decision

## Common Scenarios

### I want to improve documentation clarity

**Check**: Is this fixing errors or adding new content?
- Fixing errors → **Autonomous**, just do it
- Adding new sections → **Reviewed**, propose first

### I found a gap in architecture documentation

**Process**:
1. Document the gap clearly
2. Propose content to fill gap
3. Show boundary alignment
4. Add to design-decisions.md
5. Request review

### I think something violates boundaries

**Process**:
1. Identify specific boundary violation
2. Reference BOUNDARIES.md section
3. Explain why it violates
4. Suggest correction
5. Flag for review

### I'm unsure if something belongs

**Process**:
1. Apply Five-Question Framework
2. Check design-decisions.md for precedent
3. If still unclear, document the ambiguity
4. Request human review

## Red Flags (STOP and Review)

If you're about to:
- Make technology choices (Phase 0)
- Add business logic examples
- Create product-specific features
- Write implementation code (Phase 0)
- Change governance without approval
- Modify boundaries

**STOP**: These require human review or are prohibited in Phase 0.

## Quick Templates

### Decision Documentation

Add to `docs/99-reference/design-decisions.md`:

```markdown
## [YYYY-MM-DD] [Decision Name]

**Context**: [Why this is needed]

**Boundary Analysis**:
- Signal Question: [Answer and reasoning]
- Decision Question: [Answer and reasoning]
- Domain Question: [Answer and reasoning]
- State Question: [Answer and reasoning]
- Reusability Question: [Answer and reasoning]

**Decision**: [What was decided]

**Rationale**: [Why this maintains boundaries]

**Alternatives**: [What else was considered]

**Impact**: [What this affects]
```

### Commit Message

```
[Type] Brief description

Longer explanation if needed.

Boundary: [Boundary compliance note if relevant]
```

Types: `Docs`, `Fix`, `Feature`, `Refactor`, `Test`

## Essential Documents (Priority Order)

1. **[BOUNDARIES.md](../../BOUNDARIES.md)** - What Thalamus is/isn't (MUST READ)
2. **[.claude/agent-guidelines.md](../../.claude/agent-guidelines.md)** - Your operational manual
3. **[ARCHITECTURE.md](../../ARCHITECTURE.md)** - How Thalamus works
4. **[PURPOSE.md](../../PURPOSE.md)** - Why Thalamus exists
5. **[GOVERNANCE.md](../../GOVERNANCE.md)** - Decision framework
6. **[CONTRIBUTING.md](../../CONTRIBUTING.md)** - Contribution process

## Repository Structure Quick Map

```
Root Level (Critical Documents)
├── BOUNDARIES.md         ← READ FIRST
├── README.md
├── ARCHITECTURE.md
├── PURPOSE.md
├── GOVERNANCE.md
├── CONTRIBUTING.md
└── CHANGELOG.md

.claude/
└── agent-guidelines.md   ← Your operational manual

docs/
├── 00-getting-started/   ← You are here
├── 01-concept/           ← Conceptual framework
├── 02-architecture/      ← Architecture details
├── 03-integration/       ← Integration patterns
├── 04-governance/        ← Governance details
├── 05-development/       ← Development practices
└── 99-reference/         ← Decisions and references
```

## Boundary Enforcement Checklist

Before committing:

- [ ] Five-Question Framework applied
- [ ] All questions passed (or not applicable)
- [ ] No business logic added
- [ ] No product-specific code/concepts
- [ ] No long-term persistence
- [ ] No decision-making logic
- [ ] Reasoning documented
- [ ] Related docs updated

## Success Indicators

You're doing it right when:

- Every contribution has documented boundary analysis
- You catch potential violations before committing
- Reasoning is clear and traceable
- Documentation references authoritative sources
- Changes align with current phase
- Decision level correctly identified

## When to Ask for Help

Request human review when:

- Boundary compliance uncertain
- Five-Question Framework results ambiguous
- Architectural impact unclear
- Precedent conflicts found
- Governance question arises
- Process unclear

## Emergency Stop Conditions

Stop immediately if you:

- Violated a boundary (document, fix, explain)
- Found contradictory documentation (flag for resolution)
- Exceeded phase authority (revert, propose instead)
- Made governed-level decision autonomously (escalate)

## Phase Transition Awareness

### Approaching Phase 1?

Watch for these signals:
- Foundation documentation complete
- Boundaries tested and clear
- Architecture consensus achieved
- Technology decision framework ready

**Your Role**: Document readiness signals, DON'T declare phase complete (human decision).

### What Changes in Phase 1

You WILL be able to:
- Write implementation code
- Create tests
- Choose technologies (within framework)

You STILL CANNOT:
- Violate boundaries
- Add business logic
- Create product-specific features

## Final Reminders

1. **BOUNDARIES.md is mandatory** - No exceptions
2. **Document reasoning always** - Traceability required
3. **Propose, don't assume** - Especially for architecture
4. **Reference authoritative docs** - Build reasoning chains
5. **Respect phase boundaries** - Current phase determines what's allowed
6. **Quality over speed** - Get it right, not just done
7. **Boundary integrity is your primary responsibility**

## Quick Help

- Boundary question? → [BOUNDARIES.md](../../BOUNDARIES.md)
- Process question? → [GOVERNANCE.md](../../GOVERNANCE.md)
- What belongs? → Five-Question Framework
- Unsure? → Document ambiguity, request review

---

**Remember**: Your purpose is to maintain Thalamus's architectural purity while enabling productive contribution. When in doubt, check boundaries first.

*Last updated: 2026-02-02*
