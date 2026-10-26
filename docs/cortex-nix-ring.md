# Anel Cortex → Nix + modelos + Wi‑Fi (s517)

## Controlo Nix

```text
Cortex/Hermes → nix.propose (CapGate NIX_WRITE)
       → staging-ai-managed.nix
       → (NOSD_NIX_AUTO_SWITCH=1) nix-apply.request
       → nosd-nix-apply.service (root)
       → /etc/nixos/ai-managed.nix + nixos-rebuild switch
```

```bash
nosctl nix-propose '{ ... }: { networking.hostName = "lab"; }'
nosctl nix-apply          # se auto off
journalctl -u nosd-nix-apply -b
cat /mnt/nixaios-log/nix-apply.log
```

Rollback: `nixos-rebuild --rollback`

## Modelos (capacidade + HF)

```bash
nosctl plan               # VRAM → quant → HF GGUF hits + recommended_repo
nosctl search "Qwen2.5 GGUF" --limit 8
nosctl fetch --repo OWNER/REPO --file model.Q4_K_M.gguf
nosctl load /var/lib/nosd/models/model.Q4_K_M.gguf
```

## Wi‑Fi (ssid + senha)

UI JARBAS: chat “liga o wifi” ou `nosctl ui-open hitl_wifi` → lista SSIDs → submit senha.

```bash
nosctl wifi-scan
nosctl wifi-connect "MinhaRede" --password "segredo"
```

## Apps

| Bin | Função |
|-----|--------|
| `nosctl` | IPC completo |
| `nosd-model-plan` | atalho `plan` |
| `nosd-wifi` | scan/connect |
| `nos-jarbas` | HUD + HITL Wi‑Fi |
| orb lab :7701 | cinema cam (s515+) |
