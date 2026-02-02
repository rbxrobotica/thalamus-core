# Thalamus Architecture

**Version**: 0.1.0 | **Last Updated**: 2026-02-02 | **Phase**: Foundation

## Document Purpose

This document defines the **conceptual architecture** of Thalamus. It describes how Thalamus works at a high level, without making technology decisions. Implementation details will follow in Phase 1.

**Read First**: [BOUNDARIES.md](BOUNDARIES.md) - Understand what Thalamus is/isn't before reading architecture.

## Architectural Vision

Thalamus is a **signal mediation layer** that sits between perception and decision systems, inspired by the biological thalamus's role as the brain's sensory relay station.

```
┌──────────────────────────────────────────────────────────┐
│                    Product Layer                         │
│         (Strategos, Robson, Future Systems)              │
│                                                          │
│  • Strategic decisions                                   │
│  • Business logic                                        │
│  • Domain-specific processing                           │
└──────────────────────────────────────────────────────────┘
                          ↕ Signals with context
┌──────────────────────────────────────────────────────────┐
│                   THALAMUS LAYER                         │
│              (This Architecture)                         │
│                                                          │
│  • Signal routing                                        │
│  • Signal normalization                                  │
│  • Context management                                    │
│  • Priority handling                                     │
└──────────────────────────────────────────────────────────┘
                          ↕ Raw signals
┌──────────────────────────────────────────────────────────┐
│                 Perception Layer                         │
│          (Sensors, APIs, Data Sources)                   │
│                                                          │
│  • Market data feeds                                     │
│  • User inputs                                           │
│  • System events                                         │
│  • External APIs                                         │
└──────────────────────────────────────────────────────────┘
```

## Core Architectural Principles

### 1. Mediation, Not Decision

**Principle**: Thalamus routes and transforms signals without making strategic or business decisions.

**Implications**:
- No business rules embedded in routing logic
- No domain-specific calculations
- No strategy implementation
- Configuration-driven behavior

**Example**:
```
✅ CORRECT: Route signal based on signal.priority metadata
❌ WRONG: Route signal based on calculated portfolio risk
```

### 2. Business-Logic Agnostic

**Principle**: Thalamus works identically across all RBX products without product-specific code.

**Implications**:
- Generic signal model
- Product-agnostic interfaces
- Configurable routing rules
- No domain terminology in core

**Example**:
```
✅ CORRECT: Signal types like "urgent", "normal", "background"
❌ WRONG: Signal types like "trade_signal", "portfolio_alert"
```

### 3. Layered Responsibility

**Principle**: Clear separation between perception, mediation, and decision layers.

**Implications**:
- Thalamus doesn't implement sources or sinks
- Products provide source/sink adapters
- Thalamus provides integration interfaces
- No layer boundary violations

### 4. Short-term Context Only

**Principle**: Thalamus maintains working context, not persistent history.

**Implications**:
- Context expires after session/timeframe
- No long-term database
- Stateful during operation, ephemeral across restarts
- Products handle persistence

### 5. Reusable Core

**Principle**: Single implementation serves all RBX products.

**Implications**:
- Core library shared across products
- Product-specific adapters live outside core
- Stable API contracts
- Minimal external dependencies

## Conceptual Layers

Thalamus itself has internal layers:

```
┌─────────────────────────────────────────────────────┐
│              Integration Layer                      │
│  • Source adapters (from perception)                │
│  • Sink adapters (to decision systems)              │
│  • Configuration interfaces                         │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│              Routing Layer                          │
│  • Signal classification                            │
│  • Priority-based routing                           │
│  • Destination selection                            │
│  • Queue management                                 │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│           Normalization Layer                       │
│  • Format transformation                            │
│  • Metadata standardization                         │
│  • Signal validation                                │
│  • Contextual enrichment                            │
└─────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────┐
│              Context Layer                          │
│  • Working context maintenance                      │
│  • Attention state                                  │
│  • Recent signal history                            │
│  • Pattern tracking                                 │
└─────────────────────────────────────────────────────┘
```

### Integration Layer

**Purpose**: Interface between Thalamus and external systems.

**Responsibilities**:
- Accept signals from various sources
- Deliver signals to configured destinations
- Provide configuration interfaces
- Handle adapter lifecycle

**Boundaries**:
- Does NOT implement specific sources/sinks
- Does NOT know about product-specific protocols
- Provides generic adapter interfaces
- Products provide concrete adapters

### Routing Layer

**Purpose**: Direct signals to appropriate destinations based on metadata.

**Responsibilities**:
- Classify signals by type, priority, urgency
- Select appropriate destination(s)
- Manage signal queues
- Handle backpressure

**Boundaries**:
- Routes based on signal metadata (set by sources)
- Does NOT calculate business-relevant priorities
- Does NOT implement domain-specific routing rules
- Configurable routing policies, not hardcoded

### Normalization Layer

**Purpose**: Transform diverse signal formats into common representations.

**Responsibilities**:
- Standardize timestamps, formats, encodings
- Validate signal structure
- Add processing metadata
- Enrich with context

**Boundaries**:
- Transforms format, not semantics
- Does NOT calculate derived business values
- Does NOT implement domain-specific transformations
- Universal transformations only

### Context Layer

**Purpose**: Maintain short-term working context for signal enrichment.

**Responsibilities**:
- Track recent signal patterns
- Maintain attention/focus state
- Provide contextual metadata
- Support routing decisions

**Boundaries**:
- Short-term memory only (session-scoped)
- Does NOT persist to database
- Does NOT implement business logic
- Provides context, doesn't interpret it

## Signal Flow Architecture

### Inbound Flow (Perception → Thalamus)

```
[External Source]
      ↓
[Source Adapter] ← Provided by product
      ↓
[Integration Layer] ← Receives raw signal
      ↓
[Normalization Layer] ← Standardizes format
      ↓
[Context Layer] ← Enriches with context
      ↓
[Routing Layer] ← Determines destination
```

### Outbound Flow (Thalamus → Decision)

```
[Routing Layer] ← Selects destination
      ↓
[Integration Layer] ← Delivers signal
      ↓
[Sink Adapter] ← Provided by product
      ↓
[Decision System]
```

### Context Update Flow

```
[Normalized Signal]
      ↓
[Context Layer] ← Updates working context
      ↓
[Pattern Detection] ← Identifies patterns
      ↓
[Context Metadata] ← Available for future signals
```

## Signal Model

### Core Signal Structure

Every signal has:
- **Identity**: Unique identifier, timestamp, source
- **Type**: Classification (domain-agnostic)
- **Priority**: Urgency/importance (set by source)
- **Payload**: Actual data (opaque to Thalamus)
- **Metadata**: Processing information

```
Signal {
  // Identity
  id: unique_identifier
  timestamp: utc_timestamp
  source: source_identifier

  // Classification (set by source)
  type: signal_type
  priority: urgency_level

  // Payload (opaque to Thalamus)
  data: arbitrary_structure

  // Metadata (added by Thalamus)
  received_at: thalamus_timestamp
  normalized_at: processing_timestamp
  context: contextual_enrichment
  routing: routing_metadata
}
```

### Signal Types (Conceptual)

Thalamus recognizes **generic** signal types:
- **Event**: Something happened (state change, occurrence)
- **Request**: Action requested (query, command)
- **Response**: Reply to request (result, acknowledgment)
- **Alert**: Attention required (warning, notification)
- **Data**: Information update (stream, batch)

**Key**: These are GENERIC. Products map domain signals to these types.

**Example Mapping (Strategos)**:
```
Product Domain          → Thalamus Type
─────────────────────────────────────────
"market_price_update"   → Event (Data)
"execute_trade"         → Request
"trade_confirmation"    → Response
"risk_threshold_breach" → Alert (Event)
```

### Priority Levels (Conceptual)

- **Critical**: Immediate attention required
- **High**: Prompt processing needed
- **Normal**: Standard processing
- **Low**: Background processing
- **Deferred**: Process when idle

**Key**: Priority is SET by source systems (which have business logic), not calculated by Thalamus.

## Routing Architecture

### Routing Strategies

Thalamus supports multiple routing strategies (configured, not hardcoded):

1. **Type-based Routing**: Route by signal type
2. **Priority-based Routing**: Route by urgency level
3. **Content-based Routing**: Route by signal metadata (not payload interpretation)
4. **Broadcast Routing**: Deliver to multiple destinations
5. **Conditional Routing**: Route based on context state

**Boundary**: All routing based on metadata, never on business logic evaluation.

### Queue Management

Thalamus manages queues for:
- Priority-based ordering
- Backpressure handling
- Burst buffering
- Fair scheduling

**Boundary**: Queue policies are configurable, not business-rule driven.

### Routing Configuration

Products configure routing via:
- Routing rules (type → destination mappings)
- Priority policies (urgent signal handling)
- Queue parameters (sizes, timeouts)
- Broadcast groups (signal distribution)

**Boundary**: Configuration provided by products, not hardcoded in Thalamus.

## Context Management Architecture

### Working Context

Thalamus maintains ephemeral context:
- **Recent Signals**: Recent signal history (sliding window)
- **Attention State**: Current focus/priority areas
- **Pattern State**: Detected patterns (frequency, clustering)
- **Routing State**: Active routes and queues

### Context Enrichment

Signals enriched with:
- Recent similar signals
- Current attention state
- Pattern membership
- Temporal context (time-of-day, duration)

**Boundary**: Enrichment adds metadata, doesn't calculate business values.

### Context Lifecycle

```
Session Start
    ↓
Context Initialized (empty)
    ↓
Signals Processed → Context Updated
    ↓
Context Used for Enrichment
    ↓
Session End → Context Discarded
```

**Key**: Context is session-scoped, not persistent.

## Integration Architecture

### Source Adapter Interface

Products provide adapters that:
- Emit signals in Thalamus format
- Set appropriate type and priority
- Provide source identification
- Handle source lifecycle

**Thalamus provides**: Interface contract
**Products provide**: Concrete implementations

### Sink Adapter Interface

Products provide adapters that:
- Receive normalized signals
- Translate to product-specific format
- Handle delivery confirmation
- Manage sink lifecycle

**Thalamus provides**: Interface contract
**Products provide**: Concrete implementations

### Configuration Interface

Products configure via:
- Routing rules
- Priority policies
- Queue parameters
- Context settings

**Boundary**: Configuration expresses "what to do", not "how to decide".

## Deployment Architecture

### As a Library

Thalamus deploys as a library:
- Embedded in product processes
- No standalone deployment
- Shares product lifecycle
- Minimal resource overhead

### Multi-Product Deployment

```
┌─────────────────┐
│   Strategos     │
│  ┌───────────┐  │
│  │ Thalamus  │  │ ← Same core, Strategos config
│  └───────────┘  │
└─────────────────┘

┌─────────────────┐
│     Robson      │
│  ┌───────────┐  │
│  │ Thalamus  │  │ ← Same core, Robson config
│  └───────────┘  │
└─────────────────┘
```

**Key**: Same core implementation, different configurations.

## Architectural Patterns

### Pattern 1: Signal Enrichment Pipeline

```
Raw Signal → Normalize → Enrich → Route → Deliver
```

Each stage adds value without making decisions.

### Pattern 2: Priority-Based Routing

```
Signal → Check Priority → Select Queue → Route
```

Routing based on metadata, not business evaluation.

### Pattern 3: Context-Aware Processing

```
Signal → Query Context → Enrich Signal → Update Context
```

Context provides metadata, doesn't interpret meaning.

### Pattern 4: Configurable Behavior

```
Signal → Apply Config Rules → Execute Action
```

Configuration drives behavior, not hardcoded logic.

## Quality Attributes

### Performance

- Low-latency signal routing (microsecond scale target)
- High-throughput signal processing
- Minimal memory footprint
- Efficient queue management

### Reliability

- Graceful degradation under load
- No signal loss under normal operation
- Clear error handling
- Backpressure management

### Maintainability

- Clear layer separation
- Well-defined interfaces
- Minimal coupling
- Comprehensive testing

### Reusability

- Zero product-specific code in core
- Stable API contracts
- Backward compatibility focus
- Clear extension points

## Non-Functional Requirements

### Latency Targets (Aspirational)

- Signal normalization: < 100μs
- Routing decision: < 50μs
- Context enrichment: < 200μs
- End-to-end: < 1ms (95th percentile)

**Note**: These are targets, not requirements. Will be validated in Phase 1.

### Throughput Targets (Aspirational)

- 100K+ signals/second per instance
- Configurable based on product needs
- Graceful degradation beyond capacity

### Memory Constraints

- Minimal baseline footprint (< 50MB)
- Bounded context size (configurable)
- No unbounded growth
- Efficient signal lifecycle

## Extension Points

Future capabilities that align with boundaries:

1. **Advanced Routing**: ML-based routing (still metadata-driven)
2. **Pattern Detection**: Identify signal patterns (not interpret meaning)
3. **Adaptive Queueing**: Dynamic queue adjustment
4. **Signal Compression**: Aggregate similar signals
5. **Multi-Level Priority**: More granular priority levels

**Boundary**: All extensions maintain business-logic agnosticism.

## What's NOT in This Architecture

Deliberately excluded (align with BOUNDARIES.md):

- Business decision logic
- Domain-specific calculations
- Long-term persistence
- Standalone UI
- Product-specific features
- Strategic planning
- Risk calculation
- Trade execution
- Portfolio management

## Validation Against Boundaries

This architecture satisfies the Five-Question Framework:

1. **Signal Question**: ✅ Entirely about signal routing/normalization
2. **Decision Question**: ✅ No business decisions made
3. **Domain Question**: ✅ Completely domain-agnostic
4. **State Question**: ✅ Short-term context only
5. **Reusability Question**: ✅ Serves all RBX products identically

## Next Steps

### Phase 1 (Implementation)

When transitioning to implementation:
- Choose technology stack
- Design detailed interfaces
- Implement core layers
- Build testing framework
- Create integration examples

### Phase 2 (Integration)

- Integrate with Strategos
- Integrate with Robson
- Validate architectural assumptions
- Refine based on real-world use

### Phase 3 (Evolution)

- Extend based on product needs
- Optimize performance
- Add advanced features
- Maintain boundary integrity

## References

- [BOUNDARIES.md](BOUNDARIES.md) - Boundary definitions
- [PURPOSE.md](PURPOSE.md) - Vision and rationale
- [docs/01-concept/](docs/01-concept/) - Conceptual framework
- [docs/02-architecture/](docs/02-architecture/) - Detailed architecture

---

**Remember**: This architecture enables intelligent signal mediation without making strategic decisions. Every component respects the boundaries defined in BOUNDARIES.md.

*For implementation details, see Phase 1 documentation (when available).*
