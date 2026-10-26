# Segurança — NixAIOS

Reporte vulnerabilidades de forma responsável. Não abra issues públicas com exploits.

## Contacto

- Preferência: contacto privado com o maintainer do repositório / organização.
- Inclua: versão (`0.5.2`+), ambiente (host TCP / WSL / ISO metal), passos mínimos, impacto.

## Modelo de ameaça (resumo)

Documento completo: [`docs/SECURITY.md`](docs/SECURITY.md).

Pontos críticos:

- **CapGate server-authored** — o cliente não escolhe caps; `req.caps` ALL é ignorado.
- **Landlock / systemd** — `nosd` com `ProtectSystem=strict` + allowlist; cage **sem** `ProtectSystem=strict` (DRM).
- **Blobs** — nunca embutidos na ISO (S1.2); fetch só com deliberação Hermes + HITL JARBAS.
- **ai-managed** — `nixos-rebuild switch` não corre como user `nosd`; helper root + HITL.

## Versões suportadas

| Versão | Suporte |
|--------|---------|
| 0.5.2+ | Actual |
| anterior a 0.5 | Best-effort (scaffold) |
