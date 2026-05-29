# Manifesto das Quatro Responsabilidades da Plataforma de IA da RBX

A RBX não deve tratar IA como uma coleção de agentes, prompts, modelos ou integrações isoladas.

A RBX deve tratar IA como uma plataforma operacional governada, composta por responsabilidades separadas, auditáveis e combináveis.

A tese central é simples:

> Avaliação, controle, execução e governança não devem viver no mesmo lugar.

Quando essas responsabilidades são misturadas, a plataforma se torna frágil, difícil de auditar e perigosa para sistemas críticos. Quando são separadas, cada camada pode evoluir com clareza, mantendo rastreabilidade, segurança e capacidade de decisão.

## 1. TruthMetal: responsabilidade pela verdade

TruthMetal é a camada de groundtruth da RBX.

Sua função é responder:

> O agente está correto?

TruthMetal deve cuidar de:

- datasets de avaliação;
- golden cases;
- benchmarks;
- eval suites;
- scorers;
- métricas de qualidade;
- evidências esperadas;
- regressão de comportamento;
- relatórios de confiabilidade.

TruthMetal não é o runtime dos agentes. TruthMetal não decide produção em tempo real. TruthMetal mede, compara e produz evidência.

## 2. Thalamus: responsabilidade pelo controle

Thalamus é a camada de controle semântico e operacional da IA na RBX.

Sua função é responder:

> Este agente pode operar agora, neste contexto, com este risco?

Thalamus deve cuidar de:

- políticas de uso;
- roteamento de modelos;
- guardrails;
- validações;
- observabilidade;
- tracing;
- limites de contexto;
- gates de produção;
- fallback;
- intervenção humana;
- integração com evals e telemetry.

Thalamus não é o dono da verdade. Ele consome evidências do TruthMetal e aplica decisões operacionais.

## 3. Agent Orchestration Plane: responsabilidade pela execução

O Agent Orchestration Plane é a camada de coordenação dos agentes da RBX.

Sua função é responder:

> O que deve ser executado, por qual agente, em qual ordem, com quais condições de parada?

Ele deve cuidar de:

- catálogo de agentes;
- missões;
- planos;
- delegação;
- ordem de execução;
- condições de execução;
- retries;
- loops limitados;
- término de missão;
- memória operacional;
- sessões isoladas;
- coordenação entre agentes humanos e artificiais.

O Agent Orchestration Plane não deve ser confundido com governança, evals ou roteamento de LLM. Sua responsabilidade é coordenar trabalho.

## 4. RBX Governance: responsabilidade pela decisão institucional

RBX Governance é a camada institucional de decisão.

Sua função é responder:

> Por que decidimos isso, quem decidiu, quando, com qual impacto e quais sistemas são afetados?

RBX Governance deve cuidar de:

- ADRs;
- policies;
- standards;
- decision registry;
- mission registry;
- ownership;
- relações entre sistemas;
- supersedes e depreciações;
- auditoria arquitetural;
- rastreabilidade estratégica.

RBX Governance não executa agentes e não avalia outputs diretamente. Ela registra e organiza as decisões que tornam a plataforma compreensível e auditável.

## Separação fundamental

A RBX deve preservar esta separação:

```
TruthMetal          → O agente está correto?
Thalamus            → O agente pode operar?
Agent Orch. Plane   → O agente deve executar?
RBX Governance      → Por que decidimos isso?
```

Esta divisão impede que a plataforma de IA vire um bloco opaco.

Ela permite que a RBX construa sistemas de IA com:

- avaliação objetiva;
- controle operacional;
- execução coordenada;
- governança auditável;
- segurança em ambientes críticos;
- evolução técnica sem perda de responsabilidade.

## Princípio final

A RBX não busca apenas usar IA.

A RBX busca construir uma infraestrutura de IA confiável, governada e operacionalmente segura.

Modelos mudam. Frameworks mudam. Agentes mudam.

Mas as quatro responsabilidades permanecem:

> verdade, controle, execução e decisão.

---

## Posição deste repositório

**`thalamus-core`** É a camada de Controle da plataforma RBX.

Este repositório implementa a responsabilidade pelo controle semântico e operacional de todo tráfego AI da RBX. Qualquer chamada a modelos de linguagem, ferramentas, MCP servers ou agentes A2A dentro do ecossistema RBX deve passar por Thalamus antes e depois da execução.

Thalamus aplica políticas, valida contexto, roteia para o modelo correto, observa, faz tracing e audita. Ele não armazena memória estratégica (isso é Strategos/Governance) e não coordena missões de agentes (isso é o Agent Orchestration Plane). Ele decide se a operação AI pode acontecer agora, neste contexto, com este risco.

Toda integração LLM nova no ecossistema RBX deve ser projetada com Thalamus como portão obrigatório, nunca chamando LiteLLM ou qualquer modelo diretamente.
