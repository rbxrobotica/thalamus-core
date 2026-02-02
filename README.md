# Thalamus

**AI-First Cognitive Mediation Layer for RBX Systems**

---

## What is Thalamus?

Thalamus is a **signal mediation layer** that routes, normalizes, and contextualizes information between perception and decision-making systems. Inspired by the biological thalamus—the brain's sensory relay station—this component manages signal flow without making strategic decisions.

Think of Thalamus as a **cognitive switchboard**: it ensures the right signals reach the right decision-makers at the right time, with the right context, while remaining completely agnostic to business logic.

## Quick Navigation

### For Humans
- **New here?** Start with [docs/00-getting-started/for-humans.md](docs/00-getting-started/for-humans.md)
- **Want to contribute?** Read [CONTRIBUTING.md](CONTRIBUTING.md)
- **Understand the vision?** See [PURPOSE.md](PURPOSE.md)
- **Explore architecture?** Check [ARCHITECTURE.md](ARCHITECTURE.md)

### For AI Agents
- **Agent guidelines**: [.claude/agent-guidelines.md](.claude/agent-guidelines.md)
- **Quick start**: [docs/00-getting-started/for-agents.md](docs/00-getting-started/for-agents.md)
- **Boundaries (REQUIRED)**: [BOUNDARIES.md](BOUNDARIES.md)

### Key Documents
- [BOUNDARIES.md](BOUNDARIES.md) - **Read this first** - Defines what Thalamus IS and IS NOT
- [PURPOSE.md](PURPOSE.md) - Why Thalamus exists
- [ARCHITECTURE.md](ARCHITECTURE.md) - Conceptual architecture
- [GOVERNANCE.md](GOVERNANCE.md) - Decision-making framework
- [CONTRIBUTING.md](CONTRIBUTING.md) - How to contribute
- [CHANGELOG.md](CHANGELOG.md) - Version history

## Core Principles

### What Thalamus IS ✅
- **Signal Router**: Directs signals from sources to appropriate handlers
- **Signal Normalizer**: Transforms diverse formats into common representations
- **Context Manager**: Maintains short-term working context for signal enrichment
- **Business-Agnostic**: No product-specific logic, reusable across all RBX systems

### What Thalamus IS NOT ❌
- **Decision-Maker**: Does not implement business rules or strategies
- **Domain-Specific**: Contains no trading, portfolio, or product logic
- **Standalone App**: A library component, not an application
- **Long-term Storage**: Manages working context, not persistent data

See [BOUNDARIES.md](BOUNDARIES.md) for the complete boundary framework.

## Current Status

**Phase**: Foundation (Pre-Implementation)
**Version**: 0.1.0
**Status**: Documentation and architecture definition

We are currently establishing the conceptual foundation. No implementation code exists yet. This is intentional—we're building the architecture right before writing any code.

## Repository Structure

```
thalamus-core/
├── README.md                 # You are here
├── BOUNDARIES.md             # Boundary definitions (READ FIRST)
├── PURPOSE.md                # Vision and rationale
├── ARCHITECTURE.md           # Conceptual architecture
├── GOVERNANCE.md             # Decision framework
├── CONTRIBUTING.md           # Contribution guide
├── CHANGELOG.md              # Version history
├── LICENSE                   # Legal terms (TBD)
│
├── .claude/
│   └── agent-guidelines.md   # AI agent operational manual
│
└── docs/
    ├── 00-getting-started/   # Onboarding for humans and agents
    ├── 01-concept/           # Conceptual framework
    ├── 02-architecture/      # Architecture details
    ├── 03-integration/       # Product integration guides
    ├── 04-governance/        # Governance details
    ├── 05-development/       # Development practices
    └── 99-reference/         # References and decisions
```

## Use Cases

### Strategos (AI Trading System)
Thalamus routes market data, news signals, and risk alerts to Strategos's decision engine with proper priority and context, without implementing any trading logic.

### Robson (AI Coding Assistant)
Thalamus mediates between code change detection, test results, and decision systems, normalizing signals from different development tools.

### Future RBX Systems
Any system needing intelligent signal routing between perception and decision can integrate Thalamus without modification.

## The Biological Inspiration

The biological thalamus acts as the brain's sensory relay station:
- Receives signals from sensory organs
- Filters and prioritizes based on attention state
- Routes to appropriate cortical regions
- Does NOT interpret meaning or make decisions

Our Thalamus follows the same pattern for AI systems. See [docs/01-concept/biological-inspiration.md](docs/01-concept/biological-inspiration.md) for details.

## Key Architectural Insight

```
Perception Layer → THALAMUS → Decision Layer
   (Signals)      (Mediation)   (Actions)

Thalamus sits in the middle, providing:
- Signal normalization
- Priority-based routing
- Contextual enrichment
- Attention management

WITHOUT:
- Making decisions
- Implementing business rules
- Containing domain logic
- Acting as a standalone system
```

## Documentation Philosophy

This repository is **AI-first, human-centered**:
- Documentation designed for both humans and AI agents
- Clear boundaries enable autonomous agent work
- Explicit decision frameworks prevent scope creep
- Living documents evolve with the project

All contributors (human and AI) follow the same standards defined in [CONTRIBUTING.md](CONTRIBUTING.md) and [GOVERNANCE.md](GOVERNANCE.md).

## Getting Started

### I want to understand Thalamus
1. Read [PURPOSE.md](PURPOSE.md) - Understand why it exists
2. Read [BOUNDARIES.md](BOUNDARIES.md) - Understand what it is/isn't
3. Read [ARCHITECTURE.md](ARCHITECTURE.md) - Understand how it works
4. Explore [docs/01-concept/](docs/01-concept/) - Deep conceptual dive

### I want to contribute
1. Read [BOUNDARIES.md](BOUNDARIES.md) - **Mandatory**
2. Read [CONTRIBUTING.md](CONTRIBUTING.md) - Process and standards
3. Read [GOVERNANCE.md](GOVERNANCE.md) - Decision framework
4. Check [docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md) - Past decisions
5. Propose your contribution following the guidelines

### I want to integrate Thalamus
1. Read [ARCHITECTURE.md](ARCHITECTURE.md) - System boundaries
2. Review [docs/03-integration/](docs/03-integration/) - Integration patterns
3. See product-specific guides (Strategos, Robson, etc.)
4. Note: Implementation not yet available (Foundation phase)

### I'm an AI agent
1. Read [.claude/agent-guidelines.md](.claude/agent-guidelines.md) - **Start here**
2. Read [BOUNDARIES.md](BOUNDARIES.md) - **Mandatory before any work**
3. Read [docs/00-getting-started/for-agents.md](docs/00-getting-started/for-agents.md) - Quick reference
4. Follow the decision framework for all contributions

## Development Phases

1. **Phase 0: Foundation** (Current) - Architecture and documentation
2. **Phase 1: Implementation** - Core signal routing and normalization
3. **Phase 2: Integration** - Strategos and Robson integration
4. **Phase 3: Evolution** - Refinement based on real-world use

See [GOVERNANCE.md](GOVERNANCE.md) for phase transition criteria.

## Technology Decisions

**None yet.** We are deliberately NOT choosing technologies, languages, or frameworks during the Foundation phase. These decisions will be made when we transition to Implementation, based on the architectural foundation we're establishing now.

## Questions?

- **Architecture questions**: See [ARCHITECTURE.md](ARCHITECTURE.md) and [docs/02-architecture/](docs/02-architecture/)
- **Contribution questions**: See [CONTRIBUTING.md](CONTRIBUTING.md)
- **Boundary questions**: See [BOUNDARIES.md](BOUNDARIES.md)
- **Conceptual questions**: See [PURPOSE.md](PURPOSE.md) and [docs/01-concept/](docs/01-concept/)

## License

To be determined by RBX Systems. See [LICENSE](LICENSE) when available.

## Part of RBX Systems

Thalamus is a core component of the RBX Systems ecosystem:
- **Strategos**: AI-powered trading system
- **Robson**: AI coding assistant
- **Future systems**: Additional RBX products

Thalamus provides the cognitive mediation layer enabling these systems to process signals intelligently without embedding business logic.

---

**Remember**: Thalamus routes signals; it doesn't decide what they mean.

*Last updated: 2026-02-02*
