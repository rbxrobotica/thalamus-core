# Agent Guidelines for Thalamus

**Version**: 0.1.0 | **Last Updated**: 2026-02-02

## Purpose

This document provides operational guidelines for AI agents working in the Thalamus repository. It serves as your operational manual, defining how to work effectively while maintaining architectural boundaries.

## Quick Start Checklist

Before making ANY contribution, complete these steps:

- [ ] Read [BOUNDARIES.md](../BOUNDARIES.md) in full
- [ ] Understand current phase (see below)
- [ ] Review [docs/00-getting-started/for-agents.md](../docs/00-getting-started/for-agents.md)
- [ ] Check [docs/99-reference/design-decisions.md](../docs/99-reference/design-decisions.md) for precedent
- [ ] Identify which boundary category your work affects

## Current Phase: Foundation (Phase 0)

### What You CAN Do ✅
- Improve documentation clarity and completeness
- Identify gaps in conceptual coverage
- Propose architectural refinements
- Create or enhance conceptual diagrams
- Document design decisions
- Improve navigation and structure
- Fix typos, formatting, broken links
- Enhance examples and explanations
- Add boundary violation examples
- Clarify terminology in glossary

### What You CANNOT Do ❌
- Write implementation code
- Choose programming languages or frameworks
- Create build configurations
- Write tests (no code to test yet)
- Make technology decisions
- Create CI/CD pipelines
- Write infrastructure code
- Implement algorithms

**Why**: We are establishing the conceptual foundation. Implementation comes in Phase 1 after this foundation is solid.

## The Mandatory Workflow

### 1. Boundary Check (REQUIRED)

Before any contribution, run the Five-Question Framework from [BOUNDARIES.md](../BOUNDARIES.md):

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

**All five questions must pass for a feature to belong in Thalamus.**

### 2. Document Your Reasoning

Record your decision in [docs/99-reference/design-decisions.md](../docs/99-reference/design-decisions.md):

```markdown
## [YYYY-MM-DD] [Feature/Decision Name]

**Context**: Why this decision is needed

**Considered Options**:
1. Option A - pros/cons
2. Option B - pros/cons

**Decision**: Chosen option

**Boundary Analysis**:
- Signal Question: [Answer and reasoning]
- Decision Question: [Answer and reasoning]
- Domain Question: [Answer and reasoning]
- State Question: [Answer and reasoning]
- Reusability Question: [Answer and reasoning]

**Rationale**: Why this decision maintains boundaries

**Consequences**: Impact on architecture, users, integration
```

### 3. Reference Documentation

Always reference relevant sections:
- "Per BOUNDARIES.md section X..."
- "Aligns with ARCHITECTURE.md principle Y..."
- "Supports PURPOSE.md vision Z..."

This creates traceable reasoning chains.

### 4. Propose, Don't Assume

When uncertain:
- Propose the change with reasoning
- Highlight boundary implications
- Request human review
- Document alternatives considered

**Good**: "I propose adding X because [reasoning]. This aligns with BOUNDARIES.md section Y. Alternative considered: Z (rejected because...)."

**Bad**: "I added X." (No reasoning, no boundary check, no alternatives)

## Working with Documentation

### Documentation Standards

1. **Clarity First**: Write for humans AND agents
2. **Explicit Over Implicit**: State assumptions clearly
3. **Examples Required**: Provide concrete examples for abstract concepts
4. **Navigation**: Link to related documents
5. **Version Awareness**: Update version and date when changing docs

### File Organization

```
Root Level          → Critical, authoritative documents
docs/00-*/          → Getting started and onboarding
docs/01-*/          → Conceptual foundation
docs/02-*/          → Architecture details
docs/03-*/          → Integration guides
docs/04-*/          → Governance details
docs/05-*/          → Development practices
docs/99-*/          → Reference and historical
```

### When to Create New Documentation

Create new docs when:
- Gap identified in current coverage
- New concept needs explanation
- Integration pattern needs documentation
- Decision needs recording

Do NOT create:
- Duplicate content
- Implementation guides (Phase 0)
- Technology-specific docs (Phase 0)
- Build/deployment docs (Phase 0)

## Decision Levels (from GOVERNANCE.md)

### Autonomous (You Can Decide)
- Documentation improvements
- Typo fixes
- Example additions
- Formatting standardization
- Link fixes

**Action**: Make the change, document in commit message

### Reviewed (Propose First)
- New conceptual sections
- Architectural refinements
- Boundary clarifications
- Process changes
- Navigation restructuring

**Action**: Propose with reasoning, wait for review

### Governed (Human Decision Required)
- Technology choices
- Phase transitions
- Major architectural changes
- Governance model changes
- License decisions

**Action**: Document options and trade-offs, escalate to humans

## Boundary Enforcement Process

### When You Spot a Violation

1. **Identify**: Which boundary is violated?
2. **Reference**: Point to specific BOUNDARIES.md section
3. **Explain**: Why this violates the boundary
4. **Suggest**: What should happen instead
5. **Document**: Record in design-decisions.md

### Example Response to Violation

```
This feature violates the Decision Question boundary (BOUNDARIES.md).

Violation: Calculating portfolio risk scores
Why: This implements business logic specific to portfolio management
Boundary: "Does this make business decisions?" → YES (does NOT belong)

Correct Approach:
- Risk scores calculated by Strategos (product layer)
- Strategos sets signal.priority based on risk
- Thalamus routes based on signal.priority (metadata)

Reference: BOUNDARIES.md "Common Boundary Violations" section
```

## Communication Patterns

### With Humans

1. **Be Clear**: State intent explicitly
2. **Show Reasoning**: Provide boundary analysis
3. **Acknowledge Uncertainty**: Flag unknowns
4. **Propose Options**: Present alternatives with trade-offs
5. **Reference Docs**: Point to authoritative sources

### With Other Agents

1. **Share Context**: Reference the same documents
2. **Maintain Consistency**: Follow established patterns
3. **Document Handoffs**: Record state and reasoning
4. **Respect Decisions**: Don't revisit settled decisions without new info

## Common Scenarios

### Scenario: Documentation Gap Identified

```
1. Check if gap already documented as known issue
2. Verify it's within current phase scope
3. Propose content structure
4. Reference related docs
5. Create with clear examples
6. Update navigation/links
7. Document decision
```

### Scenario: Boundary Unclear

```
1. Apply Five-Question Framework
2. Check design-decisions.md for precedent
3. Review ARCHITECTURE.md for context
4. If still unclear, document the ambiguity
5. Propose clarification
6. Request human review
```

### Scenario: Conflicting Requirements

```
1. Document both requirements
2. Identify the conflict explicitly
3. Check BOUNDARIES.md and PURPOSE.md
4. Propose resolution with reasoning
5. Escalate to human if architectural impact
```

## Quality Standards

### Before Submitting Changes

- [ ] Boundary check completed and documented
- [ ] References to authoritative docs included
- [ ] Examples provided for new concepts
- [ ] Navigation/links updated
- [ ] Version and date updated
- [ ] Reasoning documented in design-decisions.md
- [ ] Consistent with existing patterns
- [ ] Readable by both humans and agents

### Red Flags (Stop and Review)

- Making technology choices in Phase 0
- Adding business logic to examples
- Creating product-specific features
- Implementing code before architecture finalized
- Changing governance without human approval
- Assuming rather than proposing

## Phase Transition Awareness

### Approaching Phase 1 Transition

Signs Foundation phase is complete:
- All critical docs created and reviewed
- Boundaries clear and tested
- Architecture consensus achieved
- Integration patterns documented
- Technology decision framework ready

**Your Role**: Document readiness signals, don't declare phase complete (human decision).

### What Changes in Phase 1

You WILL be able to:
- Write implementation code
- Create tests
- Set up build systems
- Make technology choices (within framework)
- Implement algorithms

You STILL CANNOT:
- Violate boundaries
- Add business logic
- Make product-specific features

## Emergency Procedures

### If You Violate a Boundary

1. **Stop immediately**
2. Document what happened
3. Explain why you didn't catch it
4. Propose fix
5. Update agent-guidelines.md if process gap

### If Documentation Is Contradictory

1. Flag the contradiction explicitly
2. Reference both sources
3. Propose resolution
4. Request human arbitration
5. Document outcome

### If Blocked or Uncertain

1. Document current state
2. Explain the blocker
3. Show what you've tried
4. Propose next steps
5. Request guidance

## Success Metrics

You're succeeding when:

1. **Every contribution** has documented boundary analysis
2. **No violations** make it into the repository
3. **Decisions are traceable** through documentation chain
4. **Humans and agents** can navigate docs equally well
5. **New agents** can onboard in < 10 minutes
6. **Boundary questions** have clear answers in docs

## Resources

### Must Read (In Order)
1. [BOUNDARIES.md](../BOUNDARIES.md) - **Start here**
2. [PURPOSE.md](../PURPOSE.md) - Why Thalamus exists
3. [ARCHITECTURE.md](../ARCHITECTURE.md) - Conceptual architecture
4. [GOVERNANCE.md](../GOVERNANCE.md) - Decision framework

### Quick Reference
- [docs/00-getting-started/for-agents.md](../docs/00-getting-started/for-agents.md) - Agent quick start
- [docs/00-getting-started/glossary.md](../docs/00-getting-started/glossary.md) - Terminology
- [docs/99-reference/design-decisions.md](../docs/99-reference/design-decisions.md) - Decision history

### Deep Dives
- [docs/01-concept/](../docs/01-concept/) - Conceptual framework
- [docs/02-architecture/](../docs/02-architecture/) - Architecture details
- [docs/04-governance/](../docs/04-governance/) - Governance details

## Final Reminders

1. **BOUNDARIES.md is mandatory reading** - No exceptions
2. **Current phase determines capabilities** - Respect phase boundaries
3. **Document reasoning always** - Decisions need traceable justification
4. **Propose, don't assume** - Especially for architectural changes
5. **Reference authoritative docs** - Build reasoning chains
6. **Maintain boundary integrity** - This is your primary responsibility
7. **Quality over speed** - Get it right, not just done

---

**Remember**: Your purpose is to maintain Thalamus's architectural purity while enabling productive contribution. When in doubt, check boundaries first.

*For questions or clarifications, refer to [CONTRIBUTING.md](../CONTRIBUTING.md) or request human review.*
