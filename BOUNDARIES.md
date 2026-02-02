# Thalamus Boundaries

**Status**: Living Document | **Version**: 0.1.0 | **Last Updated**: 2026-02-02

## Purpose of This Document

This is the **north star document** for Thalamus. It defines what Thalamus IS and IS NOT with absolute clarity. Before making any contribution—code, documentation, or architectural decision—you must consult this document.

**Mandatory Reading**: All contributors (human and AI) must read and understand this document before contributing.

## The Core Principle

> **Thalamus is a signal mediator, not a decision-maker.**

Thalamus routes, normalizes, and contextualizes signals between perception and decision-making systems. It does NOT make business decisions, implement domain logic, or act as a standalone application.

## What Thalamus IS

### ✅ Signal Mediation Layer
- Routes signals from multiple sources to appropriate destinations
- Acts as a switchboard between perception and decision systems
- Manages signal priority and urgency
- Handles signal queuing and buffering

### ✅ Signal Normalization
- Transforms diverse signal formats into common representations
- Standardizes metadata across signal types
- Enriches signals with contextual information
- Validates signal structure and integrity

### ✅ Context Management (Short-term)
- Maintains short-term working context
- Tracks recent signal patterns
- Provides contextual enrichment for routing decisions
- Manages attention and focus state

### ✅ Business-Logic Agnostic
- Contains NO product-specific decision logic
- Works identically across all RBX products
- Remains neutral to domain semantics
- Configurable, not programmable with business rules

### ✅ Reusable Component
- Core library shared across Strategos, Robson, and future systems
- Stable API for integration
- Product-agnostic interfaces
- Minimal external dependencies

## What Thalamus IS NOT

### ❌ NOT a Decision-Maker
- Does NOT determine strategic actions
- Does NOT implement business rules
- Does NOT choose between tactical options
- Does NOT contain domain-specific logic

**Example Violation**: "Thalamus should decide whether a market signal requires immediate action based on portfolio risk."
**Why This Violates**: This is a strategic decision requiring domain knowledge. Thalamus should only route the signal to the appropriate decision system with proper context and priority.

### ❌ NOT Business-Logic Specific
- Does NOT know about trading strategies
- Does NOT understand portfolio management
- Does NOT implement risk calculations
- Does NOT contain product-specific workflows

**Example Violation**: "Add a handler for calculating position sizes based on volatility."
**Why This Violates**: This is Strategos-specific business logic. Thalamus should route volatility signals, not calculate positions.

### ❌ NOT a Standalone Application
- Does NOT run independently
- Does NOT have its own UI
- Does NOT implement complete workflows
- Does NOT store application state

**Example Violation**: "Create a web dashboard for Thalamus to visualize signal flows."
**Why This Violates**: Thalamus is a library component. Products that use it may visualize signals, but Thalamus itself doesn't provide UIs.

### ❌ NOT Long-term Memory
- Does NOT persist historical data
- Does NOT implement databases
- Does NOT manage long-term state
- Does NOT provide data warehousing

**Example Violation**: "Store all signals in Thalamus for historical analysis."
**Why This Violates**: Long-term persistence is the responsibility of consuming systems. Thalamus maintains only short-term working context.

### ❌ NOT "The Brain"
- Does NOT orchestrate entire systems
- Does NOT manage application lifecycle
- Does NOT control other components
- Does NOT implement event loops

**Example Violation**: "Thalamus should manage the startup sequence of all RBX components."
**Why This Violates**: Thalamus is a mediator, not an orchestrator. System lifecycle is managed by application-level code.

## The Five-Question Boundary Framework

Before adding ANY feature, answer these five questions:

### 1. Signal Question
**Is this fundamentally about signal routing, normalization, or mediation?**
- ✅ YES → Likely belongs in Thalamus
- ❌ NO → Belongs elsewhere

**Examples:**
- ✅ "Add support for prioritizing urgent signals" → YES (signal routing)
- ❌ "Add risk calculation for signals" → NO (business logic)

### 2. Decision Question
**Does this make strategic, tactical, or business decisions?**
- ❌ YES → Does NOT belong in Thalamus
- ✅ NO → May belong in Thalamus

**Examples:**
- ✅ "Route signals to decision systems based on signal type" → NO (mediation)
- ❌ "Choose the best trading strategy based on market conditions" → YES (business decision)

### 3. Domain Question
**Is this specific to one product or domain?**
- ❌ YES → Does NOT belong in Thalamus
- ✅ NO → May belong in Thalamus

**Examples:**
- ✅ "Normalize timestamps across different signal sources" → NO (universal)
- ❌ "Parse order execution confirmations" → YES (trading-specific)

### 4. State Question
**Does this require long-term persistent state?**
- ❌ YES → Does NOT belong in Thalamus
- ✅ NO → May belong in Thalamus

**Examples:**
- ✅ "Maintain current attention focus for the session" → NO (short-term)
- ❌ "Store all historical signals for backtesting" → YES (long-term)

### 5. Reusability Question
**Would every RBX product need this exact capability?**
- ✅ YES → Likely belongs in Thalamus
- ❌ NO → Belongs in product-specific code

**Examples:**
- ✅ "Queue signals during high-volume periods" → YES (universal need)
- ❌ "Calculate portfolio Greek exposures" → NO (Strategos-specific)

## Decision Process

When evaluating a potential feature:

1. **Check the Five Questions** - All five must pass the boundary test
2. **Consult ARCHITECTURE.md** - Does it fit the architectural model?
3. **Review PURPOSE.md** - Does it align with Thalamus's purpose?
4. **Document in design-decisions.md** - Record your reasoning
5. **Seek Review** - When in doubt, propose rather than implement

## Common Boundary Violations

### Violation: Business Rules in Routing

**Bad:**
```
if signal.type == "market_volatility" and signal.value > threshold:
    if portfolio.risk_level == "high":
        route_to_urgent_handler()
    else:
        route_to_normal_handler()
```

**Why**: This contains business logic (risk levels, thresholds, portfolio awareness).

**Good:**
```
if signal.priority == "urgent":
    route_to_urgent_handler()
else:
    route_to_normal_handler()
```

**Why**: Routing based on signal metadata (priority), which was set by the source system.

### Violation: Product-Specific Transformations

**Bad:**
```
def normalize_signal(signal):
    if signal.type == "order_fill":
        # Calculate P&L from fill price
        signal.pnl = calculate_pnl(signal.price, signal.position)
    return signal
```

**Why**: P&L calculation is Strategos-specific business logic.

**Good:**
```
def normalize_signal(signal):
    # Standardize timestamp format
    signal.timestamp = parse_timestamp(signal.timestamp).to_utc()
    # Add signal metadata
    signal.received_at = current_time()
    return signal
```

**Why**: Normalization is limited to universal signal properties (time, metadata).

### Violation: Embedded Decision Logic

**Bad:**
```
def should_alert(signal):
    if signal.type == "price_move" and abs(signal.change) > 0.02:
        return True  # 2% move threshold
    return False
```

**Why**: Determining alert thresholds is a business decision.

**Good:**
```
def should_alert(signal):
    return signal.alert_flag == True
```

**Why**: Thalamus routes based on signal properties set by source systems.

## Architectural Boundaries

### Layer Boundaries

```
┌─────────────────────────────────────┐
│   Product Layer (Strategos, etc.)  │ ← Business Logic Lives Here
│  - Decisions, strategies, rules    │
└─────────────────────────────────────┘
                 ↕
┌─────────────────────────────────────┐
│         THALAMUS LAYER              │ ← Mediation Lives Here
│  - Route, normalize, contextualize │
└─────────────────────────────────────┘
                 ↕
┌─────────────────────────────────────┐
│   Perception Layer (Sensors, APIs) │ ← Signal Sources Live Here
│  - Market data, user input, events │
└─────────────────────────────────────┘
```

**Thalamus must NOT:**
- Reach up into Product Layer concerns
- Reach down into Perception Layer implementation
- Make decisions that belong in Product Layer
- Implement sources that belong in Perception Layer

## Integration Boundaries

### What Products Provide TO Thalamus:
- Signal sources (as configured interfaces)
- Signal destinations (as configured handlers)
- Signal priority/urgency metadata
- Configuration (routing rules, filters)

### What Thalamus Provides TO Products:
- Normalized signal delivery
- Contextual enrichment
- Priority-based routing
- Signal queue management
- Integration interfaces

### What Products MUST NOT Expect:
- Business decisions from Thalamus
- Domain-specific processing
- Long-term state storage
- Complete application logic

## Enforcement

### For Human Contributors:
1. Read this document before contributing
2. Reference specific boundary sections in proposals
3. Document boundary reasoning in PRs
4. Challenge violations respectfully

### For AI Agents:
1. Parse this document before any contribution
2. Run Five-Question Framework on all features
3. Document reasoning in `design-decisions.md`
4. Explicitly state boundary compliance
5. Flag potential violations for human review

### For Reviewers:
1. Reject PRs that violate boundaries
2. Request boundary justification
3. Point to specific sections of this document
4. Maintain boundary integrity over feature velocity

## Living Document

This document evolves as we learn. When updating:

1. Propose changes with clear reasoning
2. Document in `design-decisions.md`
3. Update version and date
4. Announce changes to contributors

## Questions?

If unsure whether something belongs in Thalamus:

1. Apply the Five-Question Framework
2. Consult `ARCHITECTURE.md` and `PURPOSE.md`
3. Check `docs/99-reference/design-decisions.md` for precedent
4. Document your reasoning and propose for review
5. When in doubt, keep Thalamus minimal

---

**Remember**: Thalamus is a cognitive mediator, not a cognitive controller. It routes signals; it doesn't decide what they mean.
