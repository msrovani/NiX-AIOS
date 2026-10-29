# Preços de referência — custo local vs nuvem (V2)

**Estado:** referência de ordem de grandeza, **datada**. Não é uma cotação de
fornecedor nem um benchmark. Qualquer número apresentado pela UI deriva daqui e
leva o carimbo `HYPOTHESIS ref_date=<data> ref_model=<modelo>` (ADR-0113).

## Modelo

```
custo_local_eur = joules / 3_600_000 × kwh_price_eur
custo_nuvem_usd = tokens_in/1e6 × cloud_in  +  tokens_out/1e6 × cloud_out
```

`tokens_*` são **estimativa** `chars/4` (`source = "char_heuristic"`), porque o
motor local devolve texto, não `usage`. Nunca são apresentados como contagem
do motor.

## Valores por omissão

| Campo | Default | Notas |
|-------|---------|-------|
| `kwh_price_eur` | `0.25` | tarifa doméstica média (PT, ordem de grandeza) |
| `ref_date` | `2026-10-29` | data a que estes valores se referem |
| `ref_model` | `cloud API genérica 4B–9B (referência, não benchmark)` | o contrafactual é um *modelo comparável*, não o mesmo modelo |
| `cloud_in_usd_per_mtok` | `0.60` | USD / 1M tokens de entrada |
| `cloud_out_usd_per_mtok` | `2.40` | USD / 1M tokens de saída |

**Atualizar este ficheiro é obrigatório sempre que se muda um default** em
`nos-core/src/cost.rs` ou nos módulos Nix (`services.nosd.*`).

## Overrides

Nix (`modules/neural-os.nix`):

```nix
services.nosd.kwhPriceEur = 0.23;
services.nosd.cloudUsdIn = 0.60;
services.nosd.cloudUsdOut = 2.40;
services.nosd.priceRefDate = "2026-10-29";
services.nosd.priceRefModel = "referência X";
```

Env (bancada): `NOSD_KWH_PRICE_EUR`, `NOSD_CLOUD_USD_IN`, `NOSD_CLOUD_USD_OUT`,
`NOSD_PRICE_REF_DATE`, `NOSD_PRICE_REF_MODEL`.

## Regras de honestidade

1. Valor de nuvem **sem** `ref_date` + `ref_model` = bug (não mostrar).
2. `source=tdp_estimate` (energia) também é estimativa — aparece na UI como
   `energy tdp_estimate`, nunca como medição.
3. `source=absent` = **zero** joules, não um valor plausível.
4. Não comparar com um modelo diferente sem dizer qual (`ref_model`).
5. O custo local não inclui desgaste de hardware, nem o custo do tempo humano.

## Âmbito da medição (`scope`) — importante

| `source` | `scope` | O que mede |
|----------|---------|------------|
| `rapl` / `rapl_wrap` | `cpu_package` | só o package da CPU |
| `nvml` | `gpu` | só a(s) GPU(s) via `nvidia-smi` |
| `tdp_estimate` | `machine_estimate` | máquina inteira, **estimada** (`NOSD_TDP_W`) |
| `absent` | `none` | nada (zero joules) |

Logo: o `custo_local_eur` com `scope=cpu_package` ou `gpu` é um **limite
inferior**, não o custo da máquina. Só `machine_estimate` pretende cobrir a
máquina — e é uma estimativa. A UI mostra sempre `source/scope`.
