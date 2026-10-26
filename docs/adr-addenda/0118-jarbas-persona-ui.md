# Addendum ADR-0118 — UI = domínio da persona JARBAS

**Status:** Accepted (doutrina)  
**Data:** 2026-10-23  
**Parent:** ADR-0118 · espelha bare-metal ADR-0058 (Cards) / ADR-0036 (persona)

## Cadeia cognitiva (não negociável)

```text
Humano / HW
    ↕
JARBAS  — mestre de cerimónias (persona, voz, HUD, janelas HITL)
    ↓ pede / publica intent
Hermes  — orquestrador (OPAR, skills, CapGate, mesh)
    ↓ despacha inferência / especialistas
Cortex  — gestor de IA (ModelHub, HotSwap, MoE/route, tensores)
```

A **UI não é um painel de sysadmin genérico**. É a superfície da **persona JARBAS**: ele decide *quando* mostrar, *o quê* mostrar, e *quando* pedir confirmação humana.

## Janelas = objetos de cerimónia

| Papel | Exemplos |
|-------|----------|
| **Mostrar** | estado do sistema, resposta do Hermes, progresso de fetch/modelo, mesh peers |
| **HITL** | escolher rede Wi‑Fi + senha; aprovar `nix.propose`; CapGate Escalate; confirmar load de modelo |
| **Não** | expor mmio/drivers/bare-metal; substituir Hermes/Cortex |

Contrato desejado (PORT do bare Generative Card / `UI_SPEC`):

1. Hermes/Cortex (ou skill) emitem **declaração** de janela/card (JSON / `UiDeclaration`).  
2. JARBAS **materializa** no compositor (Slint/cage hoje; cards rich = residual).  
3. Acção do humano → EventBus / IPC → Hermes (nunca Cortex a desenhar pixels).

## Conectores (prontos)

| Elo | API |
|-----|-----|
| Contrato | `nos_core::ui_window` (`WindowSpec` / `WindowAction` / topics `UI_WINDOW_*`) |
| Cortex | `suggest_window_kind(prompt)` → `HitlWifi` / `HitlConfirm` / … |
| Hermes | `WindowBroker` + `ui.open|list|close|action` IPC; chat dispara hint |
| JARBAS | Slint poll `ui.list`; `nosctl ui-*`; CapGate `HITL` só em `ui.action` |
| Backend Wi‑Fi/NM | **AWAITING** — `applied=false`, `connector_ready_backend_awaiting` |

## Estado NixAIOS (honesty)

| Peça | Status |
|------|--------|
| Doutrina JARBAS→Hermes→Cortex | **Accepted** |
| Conectores IPC + broker + hint | **IMPLEMENTED** |
| HUD Slint status + chat + poll janelas | IMPLEMENTED (thin) |
| Form Wi‑Fi pixels + NetworkManager Act | **AWAITING** |
| Generative Card pleno (0058) | residual PORT |
| Cage = só transporte DRM | sim |

## Anti-padrões

- HUD que “é o sistema” sem persona.  
- Cortex a abrir UI direta.  
- Formulários Wi‑Fi no daemon sem passar por JARBAS + CapGate HITL.  
- Confundir `jarbas-hub` slog (journal) com a cerimónia visual.
