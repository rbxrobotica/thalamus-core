# Thalamus Glossary

**Version**: 0.1.0 | **Last Updated**: 2026-02-02

## Purpose

This glossary defines key terms used throughout Thalamus documentation. Understanding these terms is essential for contributing effectively.

---

## Core Concepts

### Thalamus
The AI-first cognitive mediation layer for RBX Systems. Named after the biological thalamus, it routes, normalizes, and contextualizes signals between perception and decision systems without making business decisions.

### Signal
A discrete unit of information flowing through the system. Signals can represent events, requests, responses, alerts, or data updates. Signals have identity, type, priority, payload, and metadata.

### Mediation
The act of routing, normalizing, and contextualizing signals without making strategic or business decisions. Mediation is Thalamus's core function.

### Boundary
A clear line defining what belongs in Thalamus versus what belongs in product code. Boundaries prevent scope creep and maintain architectural integrity. See [BOUNDARIES.md](../../BOUNDARIES.md).

---

## Architectural Terms

### Perception Layer
External systems that generate signals (market data feeds, code repositories, sensors, APIs, user interfaces). Thalamus receives signals from the perception layer.

### Decision Layer
Product-specific systems that make strategic and business decisions (trading engines, code suggestion systems, action planners). Thalamus delivers signals to the decision layer.

### Signal Routing
Determining which destination(s) should receive a signal based on signal metadata (type, priority, context). Routing is configuration-driven, not business-logic driven.

### Signal Normalization
Transforming diverse signal formats into common representations. Normalization standardizes structure, timestamps, encoding, and metadata without interpreting business meaning.

### Context Management
Maintaining short-term working context that enriches signals with relevant metadata. Context includes recent signal history, attention state, and detected patterns.

### Working Context
Ephemeral, session-scoped state maintained by Thalamus to provide contextual enrichment. Working context is NOT persisted long-term.

### Signal Enrichment
Adding contextual metadata to signals (recent patterns, attention state, temporal context) without calculating business values.

---

## Boundary-Related Terms

### Business Logic
Domain-specific decision-making code (trading strategies, risk calculations, code analysis, portfolio management). Business logic does NOT belong in Thalamus.

### Business-Logic Agnostic
Containing no domain-specific knowledge or decision-making code. Thalamus is business-logic agnostic by design.

### Product-Specific
Code, configuration, or concepts that apply to only one RBX product (Strategos-specific trading logic, Robson-specific code analysis). Product-specific code does NOT belong in Thalamus core.

### Domain-Agnostic
Applicable across all domains and products. Thalamus core is domain-agnostic (works for trading, coding, healthcare, etc.).

### Five-Question Framework
The boundary validation process from [BOUNDARIES.md](../../BOUNDARIES.md). All features must pass five questions to belong in Thalamus: Signal, Decision, Domain, State, Reusability.

---

## Component Terms

### Integration Layer
Thalamus component that interfaces with external systems via source and sink adapters.

### Routing Layer
Thalamus component that classifies signals and selects destinations based on metadata.

### Normalization Layer
Thalamus component that transforms signal formats and validates structure.

### Context Layer
Thalamus component that maintains working context and provides enrichment metadata.

### Source Adapter
Product-provided component that emits signals into Thalamus in standard format.

### Sink Adapter
Product-provided component that receives normalized signals from Thalamus.

---

## Signal-Related Terms

### Signal Type
Classification of signal purpose (Event, Request, Response, Alert, Data). Types are generic, not domain-specific.

### Signal Priority
Urgency level set by source systems (Critical, High, Normal, Low, Deferred). Priority drives routing decisions.

### Signal Payload
The actual data carried by a signal. Payload is opaque to Thalamus—Thalamus routes it without interpreting it.

### Signal Metadata
Processing information added by Thalamus (received_at, normalized_at, context, routing). Metadata is distinct from payload.

### Signal Identity
Unique identifier, timestamp, and source information that distinguishes each signal.

---

## Process Terms

### Phase
Development stage with defined objectives and allowed activities. Current phases: Foundation (0), Implementation (1), Integration (2), Evolution (3). See [GOVERNANCE.md](../../GOVERNANCE.md).

### Decision Level
Authority level required for a change: Autonomous (agent decides), Reviewed (human reviews), Governed (human decides). See [GOVERNANCE.md](../../GOVERNANCE.md).

### Boundary Violation
A contribution that violates the definitions in [BOUNDARIES.md](../../BOUNDARIES.md) by adding business logic, product-specific code, decision-making, long-term storage, or non-reusable features.

### Design Decision
A documented choice about architecture, features, or process recorded in [docs/99-reference/design-decisions.md](../99-reference/design-decisions.md) with context, reasoning, and boundary analysis.

---

## RBX Systems Terms

### RBX Systems
The organization building Thalamus and AI-powered products (Strategos, Robson, future systems).

### Strategos
RBX Systems' AI-powered trading system. Strategos uses Thalamus for signal mediation between market perception and trading decisions.

### Robson
RBX Systems' AI coding assistant. Robson uses Thalamus for signal mediation between code events and coding decisions.

---

## Technical Terms (Phase 1+)

*These terms will be defined when implementation begins.*

### Queue
(To be defined in Phase 1)

### Backpressure
(To be defined in Phase 1)

### Adapter Interface
(To be defined in Phase 1)

### Configuration Schema
(To be defined in Phase 1)

---

## Biological Terms

### Biological Thalamus
Brain structure that acts as sensory relay station, routing signals from sensory organs to appropriate cortical regions. The biological thalamus inspired Thalamus architecture.

### Cortical Region
Area of brain cortex that processes specific types of information (visual cortex, auditory cortex). Analogous to decision systems in Thalamus architecture.

### Sensory Relay
Function of routing sensory information without interpreting meaning. This is the core function Thalamus adopts from neuroscience.

### Attention State
Biological mechanism for prioritizing certain signals. Thalamus maintains attention state as part of working context.

---

## Governance Terms

### Autonomous Decision
Change that AI agents can make without human approval (typos, formatting, examples). See [GOVERNANCE.md](../../GOVERNANCE.md).

### Reviewed Decision
Change requiring human review before implementation (new features, architectural changes). See [GOVERNANCE.md](../../GOVERNANCE.md).

### Governed Decision
Strategic decision requiring explicit human authority (technology choices, phase transitions, boundary changes). See [GOVERNANCE.md](../../GOVERNANCE.md).

### Phase Transition
Moving from one development phase to the next (Foundation → Implementation → Integration → Evolution). Requires human approval.

### Completion Criteria
Required conditions for declaring a phase complete and transitioning to the next phase. See [GOVERNANCE.md](../../GOVERNANCE.md).

---

## Contribution Terms

### AI-First, Human-Centered
Development philosophy where AI agents contribute autonomously within clear boundaries, while humans make strategic decisions and maintain architectural integrity.

### Boundary Discipline
The practice of rigorously maintaining architectural boundaries by rejecting contributions that violate [BOUNDARIES.md](../../BOUNDARIES.md).

### Proposal
Documented suggestion for a reviewed-level or governed-level change, including context, reasoning, alternatives, and boundary analysis.

### Precedent
Past design decision documented in [docs/99-reference/design-decisions.md](../99-reference/design-decisions.md) that informs current decisions.

---

## Anti-Patterns (What NOT to Do)

### Boundary Creep
Gradually adding features that violate boundaries, often justified as "small exceptions" or "edge cases."

### Business Logic Infiltration
Adding domain-specific decision-making code to Thalamus under the guise of "smart routing" or "intelligent processing."

### Product Coupling
Creating dependencies between Thalamus core and specific products (Strategos, Robson), violating reusability.

### Premature Optimization
Making technology decisions or implementing code during Phase 0 (Foundation), before architecture is finalized.

---

## Quality Attributes

### Latency
Time between signal input and signal delivery. Thalamus targets low latency (microsecond to millisecond scale).

### Throughput
Number of signals processed per unit time. Thalamus targets high throughput (100K+ signals/second).

### Reusability
Ability to use the same core code across all RBX products without modification. Core design principle.

### Maintainability
Ease of understanding, modifying, and extending code. Enhanced by clear boundaries and layer separation.

### Reliability
Consistent, predictable behavior under various conditions. Includes graceful degradation and error handling.

---

## Document References

### Authoritative Documents
The root-level documents that define Thalamus ([BOUNDARIES.md](../../BOUNDARIES.md), [ARCHITECTURE.md](../../ARCHITECTURE.md), [PURPOSE.md](../../PURPOSE.md), [GOVERNANCE.md](../../GOVERNANCE.md), [CONTRIBUTING.md](../../CONTRIBUTING.md)).

### Agent Guidelines
[.claude/agent-guidelines.md](../../.claude/agent-guidelines.md) - Operational manual for AI agents working in Thalamus repository.

### Design Decisions
[docs/99-reference/design-decisions.md](../99-reference/design-decisions.md) - Historical record of architectural and design decisions with reasoning.

---

## Usage Examples

### Good Usage (Aligned with Boundaries)

**Signal**: "Market price update received from exchange API"
**Normalization**: "Convert exchange timestamp to UTC, validate structure"
**Routing**: "Route to decision system based on signal.priority"
**Context**: "Enrich with recent price pattern metadata"

### Bad Usage (Boundary Violations)

**Business Logic**: "Calculate if price change exceeds risk threshold" (Decision boundary violation)
**Product-Specific**: "Parse Strategos-specific order format" (Domain boundary violation)
**Long-term Storage**: "Store all signals in database permanently" (State boundary violation)
**Decision-Making**: "Determine if trade should be executed" (Decision boundary violation)

---

## Common Confusions

### "Thalamus is a message bus"
**Clarification**: Thalamus is more than transport—it understands signal semantics (priority, type, context) while remaining business-agnostic.

### "Thalamus makes routing decisions"
**Clarification**: Thalamus routes based on metadata SET by source systems. It doesn't calculate what priority should be.

### "Any signal processing belongs in Thalamus"
**Clarification**: Only business-agnostic processing (normalization, routing, context enrichment). Business-specific processing belongs in products.

### "Thalamus is product-specific"
**Clarification**: Thalamus core is universal. Products provide adapters and configuration.

---

## Related Resources

- **Detailed Definitions**: See [docs/99-reference/terminology.md](../99-reference/terminology.md) (when created)
- **Conceptual Background**: See [docs/01-concept/](../01-concept/) (when created)
- **Architecture Details**: See [ARCHITECTURE.md](../../ARCHITECTURE.md)
- **Boundary Framework**: See [BOUNDARIES.md](../../BOUNDARIES.md)

---

## Contributing to This Glossary

To add or modify terms:

1. Ensure term is used in Thalamus documentation
2. Verify definition aligns with [BOUNDARIES.md](../../BOUNDARIES.md)
3. Use clear, concise language
4. Provide examples when helpful
5. Link to authoritative documents
6. Update version and date

**Decision Level**: Autonomous for clarifications, Reviewed for new terms

---

**Last Updated**: 2026-02-02 | **Version**: 0.1.0

*This glossary is a living document. Suggest improvements via the contribution process defined in [CONTRIBUTING.md](../../CONTRIBUTING.md).*
