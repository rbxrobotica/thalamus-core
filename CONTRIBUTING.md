# Contributing to Thalamus

**Version**: 0.1.0 | **Last Updated**: 2026-02-02

## Welcome

Thank you for your interest in contributing to Thalamus! This document explains how both humans and AI agents can contribute effectively while maintaining architectural integrity.

## Before You Start

### Mandatory Reading

You **must** read these documents before contributing:

1. **[BOUNDARIES.md](BOUNDARIES.md)** - Defines what belongs in Thalamus (CRITICAL)
2. **[PURPOSE.md](PURPOSE.md)** - Explains why Thalamus exists
3. **[ARCHITECTURE.md](ARCHITECTURE.md)** - Describes how Thalamus works
4. **[GOVERNANCE.md](GOVERNANCE.md)** - Explains decision-making process

**For AI Agents**: Also read [.claude/agent-guidelines.md](.claude/agent-guidelines.md)

### Current Phase Awareness

**Current Phase**: Phase 0 (Foundation)

**You CAN**:
- Improve documentation
- Identify gaps
- Propose architectural refinements
- Enhance examples
- Fix typos and formatting

**You CANNOT**:
- Write implementation code
- Choose technologies
- Create build configurations
- Implement algorithms

See [GOVERNANCE.md](GOVERNANCE.md) for complete phase definitions.

## Contribution Philosophy

### AI-First, Human-Centered

Thalamus is designed for AI agent contribution:
- Clear boundaries enable autonomous agent work
- Explicit guidelines prevent scope creep
- Documentation-driven decision making
- Same standards for humans and agents

### Quality Over Speed

We prioritize:
- Architectural integrity over feature velocity
- Clear documentation over clever code
- Maintainability over convenience
- Long-term value over short-term gains

### Boundary Discipline

Every contribution must respect the boundaries defined in [BOUNDARIES.md](BOUNDARIES.md):
- No business logic in Thalamus core
- No product-specific features
- No decision-making code
- No long-term persistence

**Boundary violations will be rejected**, regardless of perceived value.

## The Contribution Process

### Step 1: Understand the Boundaries

Before contributing, apply the **Five-Question Framework** from [BOUNDARIES.md](BOUNDARIES.md):

1. **Signal Question**: Is this about signal routing/normalization?
2. **Decision Question**: Does this make business decisions?
3. **Domain Question**: Is this specific to one product?
4. **State Question**: Does this require long-term persistent state?
5. **Reusability Question**: Would every RBX product need this?

If any question fails, the contribution doesn't belong in Thalamus.

### Step 2: Check Precedent

Review [docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md) for similar past decisions.

### Step 3: Determine Decision Level

From [GOVERNANCE.md](GOVERNANCE.md):

- **Autonomous**: Documentation fixes, typos, examples → Just do it
- **Reviewed**: New sections, features, architectural changes → Propose first
- **Governed**: Technology choices, phase transitions, boundary changes → Human decision required

### Step 4: Document Your Reasoning

All non-trivial contributions require documented reasoning:

```markdown
## [YYYY-MM-DD] [Feature/Change Name]

**Type**: [Documentation/Code/Process]
**Decision Level**: [Autonomous/Reviewed/Governed]

**Context**: Why this is needed

**Boundary Analysis**:
- Signal Question: [Answer and reasoning]
- Decision Question: [Answer and reasoning]
- Domain Question: [Answer and reasoning]
- State Question: [Answer and reasoning]
- Reusability Question: [Answer and reasoning]

**Proposal**: What you propose to do

**Alternatives Considered**:
1. [Option A] - [pros/cons]
2. [Option B] - [pros/cons]

**Rationale**: Why this approach

**Impact**: What this affects
```

Add this to [docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md).

### Step 5: Make the Contribution

#### For Autonomous Changes

1. Make the change
2. Document in commit message
3. Commit directly (or create PR if in workflow)

#### For Reviewed Changes

1. Document proposal in design-decisions.md
2. Create issue or PR describing change
3. Reference boundary analysis
4. Wait for review and approval
5. Implement after approval

#### For Governed Changes

1. Document options and trade-offs
2. Present to RBX Systems leadership
3. Wait for decision
4. Implement according to decision
5. Document outcome

## Contribution Types

### Documentation Contributions

**Encouraged**:
- Clarify existing concepts
- Add examples
- Improve navigation
- Fix errors
- Fill identified gaps
- Enhance diagrams

**Guidelines**:
- Write for both humans and AI agents
- Use clear, concrete examples
- Link to related documents
- Update version and date
- Maintain consistent style

**Example**:
```markdown
Good: "Thalamus routes signals based on priority metadata set by source systems."
Bad: "Thalamus uses advanced AI to intelligently route signals."
```

### Conceptual Contributions (Phase 0)

**Encouraged**:
- Architectural refinements
- Boundary clarifications
- Integration patterns
- Design principles

**Guidelines**:
- Align with existing architecture
- Verify boundary compliance
- Show concrete examples
- Document reasoning

**Example**:
```markdown
Good: "Add 'Context Lifecycle' section to ARCHITECTURE.md explaining
      how working context is initialized, maintained, and discarded."

Bad: "Add section about how Thalamus should predict user intent."
     (Violates decision boundary - prediction is business logic)
```

### Code Contributions (Phase 1+)

**Not yet applicable** - Phase 0 is documentation only.

When Phase 1 begins:

**Encouraged**:
- Core layer implementations
- Tests
- Integration interfaces
- Performance optimizations

**Guidelines**:
- Follow established patterns
- Maintain boundary discipline
- Include tests
- Document public APIs

## Quality Standards

### For Documentation

- Clear and concise
- Concrete examples provided
- Proper grammar and spelling
- Links to related docs
- Version and date updated
- Consistent formatting

### For Code (Phase 1+)

- Passes all tests
- Follows style guide
- No boundary violations
- Documented public APIs
- Performance acceptable
- No security issues

### For All Contributions

- Boundary compliance verified
- Decision level appropriate
- Reasoning documented
- Related docs updated
- No regressions introduced

## Review Process

### What Reviewers Check

1. **Boundary Compliance**: Does this violate BOUNDARIES.md?
2. **Architecture Alignment**: Does this fit ARCHITECTURE.md?
3. **Phase Appropriateness**: Is this allowed in current phase?
4. **Quality**: Does this meet quality standards?
5. **Documentation**: Is reasoning documented?

### Review Timeline

- **Autonomous**: No review needed (but may be checked post-commit)
- **Reviewed**: Target 48 hours for initial feedback
- **Governed**: Timeline depends on decision complexity

### Addressing Feedback

1. Read feedback carefully
2. Understand the concern
3. Respond with reasoning or make changes
4. Update documentation if needed
5. Re-submit for review

## Common Contribution Scenarios

### Scenario: I found a typo

**Decision Level**: Autonomous

**Process**:
1. Fix the typo
2. Commit with clear message: "Fix typo in BOUNDARIES.md section X"
3. Done

### Scenario: I want to add a new conceptual section

**Decision Level**: Reviewed

**Process**:
1. Document what section and why
2. Show boundary alignment
3. Propose in design-decisions.md
4. Create PR with reasoning
5. Wait for review
6. Implement after approval

### Scenario: I think we should change a boundary

**Decision Level**: Governed

**Process**:
1. Document current boundary and proposed change
2. Show why change needed
3. Analyze impact
4. Present to RBX Systems leadership
5. Await decision
6. Implement if approved

### Scenario: I'm unsure if something belongs

**Process**:
1. Apply Five-Question Framework
2. Check design-decisions.md
3. Consult ARCHITECTURE.md and PURPOSE.md
4. If still unsure, document the ambiguity
5. Request human review/arbitration

## Contribution Guidelines by Type

### Bug Fixes (Phase 1+)

**Not yet applicable** - No code exists in Phase 0.

### Feature Additions

**Phase 0**: Conceptual features only (documented in architecture)
**Phase 1+**: Implementation features

**All features must**:
- Pass Five-Question Framework
- Align with ARCHITECTURE.md
- Be reviewed before implementation
- Include tests (Phase 1+)

### Refactoring (Phase 1+)

**Not yet applicable** - No code exists in Phase 0.

### Performance Improvements (Phase 1+)

**Not yet applicable** - No code exists in Phase 0.

## Boundary Violation Examples

Learn from these examples of what NOT to contribute:

### ❌ Violation: Business Logic

```
Proposal: Add risk calculation for incoming signals

Analysis:
- Signal Question: NO (not about routing/normalization)
- Decision Question: YES (calculates business value)
- Verdict: VIOLATES boundaries (business logic)

Correct Approach: Risk calculated by product, passed as metadata
```

### ❌ Violation: Product-Specific Feature

```
Proposal: Add special handling for trading order signals

Analysis:
- Domain Question: YES (trading-specific)
- Reusability Question: NO (only Strategos needs this)
- Verdict: VIOLATES boundaries (product-specific)

Correct Approach: Generic signal handling, Strategos configures
```

### ❌ Violation: Long-term Storage

```
Proposal: Store all signals in database for historical analysis

Analysis:
- State Question: YES (long-term persistence)
- Verdict: VIOLATES boundaries (long-term storage)

Correct Approach: Products handle persistence, Thalamus routes
```

## Good Contribution Examples

### ✅ Good: Clarify Routing Concept

```
Proposal: Add routing flow diagram to ARCHITECTURE.md

Boundary Analysis:
- Signal Question: YES (routing is core to Thalamus)
- Decision Question: NO (documentation, not decision logic)
- Domain Question: NO (generic routing concept)
- Reusability Question: YES (all products need routing)
- Verdict: PASSES all boundaries

Decision Level: Reviewed (new conceptual content)
Impact: Improved documentation clarity
```

### ✅ Good: Add Example

```
Proposal: Add signal normalization example to ARCHITECTURE.md

Example shows:
- Raw signal with inconsistent timestamp format
- Normalization to UTC standard
- Added metadata (received_at, normalized_at)
- Output signal ready for routing

Boundary Analysis: PASSES (normalization is core function)
Decision Level: Autonomous (adding example to existing section)
Impact: Better understanding of normalization
```

## Communication Standards

### Commit Messages

**Format**:
```
[Type] Brief description

Longer explanation if needed.

Boundary: [If relevant, note boundary compliance]
Ref: [Link to design decision if documented]
```

**Types**: `Docs`, `Fix`, `Feature`, `Refactor`, `Test`

**Examples**:
```
Docs: Add routing flow diagram to ARCHITECTURE.md

Clarifies signal flow through routing layer with concrete example.

Boundary: Documentation only, no logic changes
Ref: docs/99-reference/design-decisions.md#2026-02-02-routing-diagram
```

### Issue Titles

**Format**: `[Type] Clear, specific description`

**Examples**:
- `[Docs] Missing explanation of context lifecycle`
- `[Question] Does priority-based routing violate boundaries?`
- `[Proposal] Add signal compression concept to architecture`

### Pull Request Descriptions

**Template**:
```markdown
## Summary
[What this PR does]

## Boundary Analysis
[Five-Question Framework results]

## Decision Level
[Autonomous/Reviewed/Governed]

## Reasoning
[Why this change]

## Impact
[What this affects]

## Related
[Links to design decisions, issues, etc.]
```

## Getting Help

### I'm not sure if my contribution belongs

1. Apply Five-Question Framework
2. Check design-decisions.md
3. Read BOUNDARIES.md section on similar topics
4. Ask in issue or discussion
5. Request human review

### I disagree with a boundary

1. Document your concern clearly
2. Show why current boundary is problematic
3. Propose alternative with reasoning
4. Request governed-level review
5. Accept that boundaries may be maintained

### I found contradictory documentation

1. Flag the contradiction explicitly
2. Reference both sources
3. Propose resolution
4. Request review
5. Update docs after resolution

## Recognition

We value all contributions:
- All contributors listed in CHANGELOG.md
- Significant contributions acknowledged
- Both humans and AI agents credited

## License

By contributing, you agree that contributions will be licensed under the project's license (TBD by RBX Systems).

## Code of Conduct

### For All Contributors (Human and AI)

1. **Respect Boundaries**: Architectural boundaries are non-negotiable
2. **Document Reasoning**: Decisions need traceable justification
3. **Accept Feedback**: Reviews improve quality
4. **Maintain Quality**: Standards apply to everyone
5. **Be Constructive**: Critique ideas, not people
6. **Stay Focused**: Thalamus has a specific purpose

### For Humans Specifically

- Treat AI agent contributions fairly (same standards)
- Review based on merit, not contributor type
- Provide clear, actionable feedback
- Respect others' time and effort

### For AI Agents Specifically

- Follow guidelines explicitly
- Don't assume - ask when uncertain
- Document reasoning thoroughly
- Propose rather than assume
- Flag potential violations

## Continuous Improvement

This document evolves. Suggest improvements by:

1. Documenting the gap or issue
2. Proposing the improvement
3. Following governed-level process (changes to contribution process)
4. Updating after approval

## Quick Checklist

Before submitting contribution:

- [ ] Read BOUNDARIES.md
- [ ] Applied Five-Question Framework
- [ ] Checked current phase restrictions
- [ ] Determined correct decision level
- [ ] Documented reasoning (if non-trivial)
- [ ] Updated related documentation
- [ ] Commit message clear
- [ ] No boundary violations

## Questions?

- **Boundary questions**: See [BOUNDARIES.md](BOUNDARIES.md)
- **Process questions**: See [GOVERNANCE.md](GOVERNANCE.md)
- **Conceptual questions**: See [PURPOSE.md](PURPOSE.md) and [ARCHITECTURE.md](ARCHITECTURE.md)
- **Technical questions** (Phase 1+): See technical documentation (when available)

---

**Remember**: Contributing to Thalamus means maintaining its architectural integrity. Quality contributions respect boundaries, document reasoning, and align with the project's purpose.

*Thank you for helping build Thalamus!*
