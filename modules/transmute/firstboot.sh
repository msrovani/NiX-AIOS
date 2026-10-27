#!/usr/bin/env bash
# Firstboot texto — modules/transmute-stage.nix (0.5.2).
# Placeholders: @utilLinux@ @networkmanager@ @iproute2@ @kbd@ @curl@ @systemd@ @nosd@ @python3@
    set -eu
    # NixOS live: pkgs.sudo/bin/sudo NAO tem setuid — so /run/wrappers/bin/sudo
    SUDO=/run/wrappers/bin/sudo
    if [ ! -x "$SUDO" ]; then SUDO=/run/current-system/sw/bin/sudo; fi
    if [ ! -x "$SUDO" ]; then SUDO=sudo; fi
    export PATH="/run/wrappers/bin:/run/current-system/sw/bin:$PATH"

    # ---- Modo (0.6 alpha.5) ----------------------------------------------
    # A ISO arranca com console=ttyS0 e o auto-login cai em /dev/ttyS0: sem
    # isto o firstboot nunca corria (os padrões só viam /dev/tty[0-9]*).
    # Serial/hvc = lab/headless -> não bloquear à espera de input.
    TTY_DEV=$(tty 2>/dev/null || echo unknown)
    AUTO=0
    case "${NOSD_FIRSTBOOT_AUTO:-0}" in 1|yes|true) AUTO=1 ;; esac
    case "${NOSD_FIRSTBOOT_ASK:-0}" in 1|yes|true) AUTO=0 ;; esac
    if [ "${NOSD_FIRSTBOOT_ASK:-0}" != "1" ] && [ "$AUTO" != "1" ]; then
      case "$TTY_DEV" in
        /dev/ttyS*|/dev/hvc*) AUTO=1 ;;
      esac
      # Sem terminal controlável (systemd/CI): não pendurar o boot.
      if [ ! -c /dev/tty ]; then AUTO=1; fi
    fi
    NET_WAIT=${NOSD_FIRSTBOOT_NET_WAIT:-30}
    if [ "$AUTO" = "1" ]; then NET_WAIT=8; fi

    # 0.5.x: FAT OFF — log /tmp + persist /var/lib/nosd (Linux FS)
    LOG=/tmp/nos-firstboot.log
    touch "$LOG" 2>/dev/null || LOG=/tmp/firstboot-$USER.log
    log() {
      echo "$@" | tee -a "$LOG" >/dev/tty 2>/dev/null || echo "$@" > /dev/tty
      echo "$@" | $SUDO tee -a /var/lib/nosd/firstboot.log >/dev/null 2>&1 || true
    }
    ask() { printf '%s' "$1" > /dev/tty; read -r "$2" < /dev/tty || eval "$2="; }
    # $1=prompt $2=var $3=default (modo auto). Sem default => vazio.
    ask_def() {
      if [ "$AUTO" = "1" ]; then
        printf -v "$2" '%s' "${3:-}"
        log "   (auto) ${1}-> ${3:-<vazio>}"
        return 0
      fi
      printf '%s' "$1" > /dev/tty
      read -r "$2" < /dev/tty || eval "$2="
    }

    # Lock em /tmp (nixos nao cria em /run sem perms)
    LOCK=/tmp/nos-firstboot.lock
    exec 9>"$LOCK"
    if ! @utilLinux@/bin/flock -n 9; then
      echo "nos-firstboot ja em curso — a sair" > /dev/tty
      exit 0
    fi

    $SUDO mkdir -p /var/lib/nosd /var/lib/nosd/models 2>/dev/null || true
    $SUDO chmod 1777 /var/lib/nosd 2>/dev/null || $SUDO chmod a+rwx /var/lib/nosd 2>/dev/null || true
    $SUDO touch /var/lib/nosd/firstboot.log 2>/dev/null || true
    $SUDO chmod a+rw /var/lib/nosd/firstboot.log 2>/dev/null || true
    echo "firstboot: persist=/var/lib/nosd (FAT OFF (0.5.x))" > /dev/tty

    log "=== nos-firstboot start $(date -Is 2>/dev/null || echo now) tty=$TTY_DEV log=$LOG ==="
    log "modo=$([ "$AUTO" = 1 ] && echo auto || echo interactivo) net_wait=${NET_WAIT}s"

    if [ -f /var/lib/nosd/firstboot.done ]; then
      log "firstboot.done presente — a sair"
      exit 0
    fi
    if [ -f /var/lib/nosd/aios-mode ]; then
      mode=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode || true)
      if [ "$mode" = "aios" ]; then
        log "aios-mode=aios — hotswap deve tratar; a sair"
        exit 0
      fi
    fi

    # persist_ok = Linux FS writable (nao depende de FAT)
    persist_ok=0
    if $SUDO test -w /var/lib/nosd 2>/dev/null; then
      persist_ok=1
    fi

    save_file() {
      # $1=name $2=content — so /var/lib/nosd (0.5.0: falha visível se tee?0)
      if ! printf '%s\n' "$2" | $SUDO tee "/var/lib/nosd/$1" >/dev/null; then
        log "ERRO: nao gravou /var/lib/nosd/$1 (permissao/sudo)"
        return 1
      fi
      $SUDO chmod a+rw "/var/lib/nosd/$1" 2>/dev/null || true
      return 0
    }
    step_file=/var/lib/nosd/firstboot.step
    set_step() {
      if ! printf '%s\n' "$1" | $SUDO tee "$step_file" >/dev/null; then
        log "ERRO: nao gravou firstboot.step=$1"
        return 1
      fi
      $SUDO chmod a+rw "$step_file" 2>/dev/null || true
      return 0
    }
    cur_step=0
    if [ -f "$step_file" ]; then
      cur_step=$(tr -d ' \n\r' < "$step_file" || echo 0)
    fi
    case "$cur_step" in (*[!0-9]*|"") cur_step=0 ;; esac

    net_connected() {
      @networkmanager@/bin/nmcli -t -f STATE g 2>/dev/null | grep -qi '^connected' \
        || @networkmanager@/bin/nmcli -t -f DEVICE,TYPE,STATE device 2>/dev/null | grep -qE ':(ethernet|wifi):connected'
    }

    log ""
    log "=============================================="
    log "  Neural AIOS — configuracao inicial (texto)"
    log "=============================================="
    log " Persistencia Linux: $([ "$persist_ok" = 1 ] && echo OK || echo FALHA)  passo_salvo=$cur_step  dir=/var/lib/nosd"
    log ""

    # ---- Q1 rede (PRIMEIRO: wired/wireless) ----
    NET_OK=0
    if [ "$cur_step" -ge 1 ]; then
      log "1) Rede — ja configurada (resume)"
      net_connected && NET_OK=1 || true
    else
      log "1) Rede (cabo ou Wi-Fi) — necessario para baixar idioma/voz/modelos"
      $SUDO systemctl start NetworkManager.service 2>/dev/null || true
      sleep 1
      if net_connected; then
        DEV=$(@networkmanager@/bin/nmcli -t -f DEVICE,TYPE,STATE device 2>/dev/null | awk -F: '$3=="connected"{print $1" ("$2")"; exit}')
        IP=$(@iproute2@/bin/ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}' || true)
        SSID=$(@networkmanager@/bin/nmcli -t -f ACTIVE,SSID dev wifi 2>/dev/null | awk -F: '$1=="yes"{print $2; exit}' || true)
        log "   Ja ligado: ${DEV:-?}${SSID:+ wifi=$SSID}${IP:+ src=$IP}"
        NET_OK=1
        [ -n "${SSID:-}" ] && save_file wifi-ssid "$SSID"
        save_file network.conf "connected=1"
      else
        log "   Sem ligacao."
        log "   1) Wi-Fi (listar SSIDs)"
        log "   2) Esperar cabo Ethernet (30s)"
        log "   3) Continuar offline (sem download HF)"
        ask_def "Escolha [1]: " ans 1
        case "${ans:-1}" in
          2)
            log "   Aguardando Ethernet…"
            for i in $(seq 1 $NET_WAIT); do
              if net_connected; then NET_OK=1; log "   Cabo OK (t=$i)"; break; fi
              sleep 1
            done
            if [ "$NET_OK" != "1" ]; then
              log "   Timeout cabo. Continuar offline? [S/n]"
              ask_def "> " cont S
              case "${cont:-S}" in [Nn]*) log "Abortado."; exit 1 ;; esac
              save_file network.conf "connected=0"
            else
              save_file network.conf "connected=1"
            fi
            ;;
          3)
            log " ? offline"
            save_file network.conf "connected=0"
            ;;
          *)
            $SUDO @networkmanager@/bin/nmcli device wifi rescan 2>/dev/null || true
            sleep 2
            mapfile -t WIFI_LIST < <(@networkmanager@/bin/nmcli -t -f SSID,SIGNAL,SECURITY device wifi list 2>/dev/null | awk -F: 'NF>=1 && $1!=""{print}' | head -20)
            if [ "${#WIFI_LIST[@]}" -eq 0 ]; then
              log "   Nenhum SSID. Offline? [S=sim / n=abortar]"
              ask_def "> " skip S
              case "${skip:-S}" in [Nn]*) exit 1 ;; esac
              save_file network.conf "connected=0"
            else
              i=1
              for line in "${WIFI_LIST[@]}"; do
                ssid=$(printf '%s' "$line" | cut -d: -f1)
                sig=$(printf '%s' "$line" | cut -d: -f2)
                sec=$(printf '%s' "$line" | cut -d: -f3-)
                log "$(printf "   %2d) %s  (%s%% %s)" "$i" "$ssid" "$sig" "$sec")"
                i=$((i+1))
              done
              log "   S) offline"
              ask_def "Escolha: " pick S
              if [ "$pick" = "S" ] || [ "$pick" = "s" ] || [ -z "${pick:-}" ]; then
                save_file network.conf "connected=0"
              elif [ "$pick" -ge 1 ] 2>/dev/null && [ "$pick" -le "${#WIFI_LIST[@]}" ]; then
                SSID=$(printf '%s' "${WIFI_LIST[$((pick-1))]}" | cut -d: -f1)
                printf "Senha Wi-Fi '%s' (eco oculto): " "$SSID" > /dev/tty
                stty -echo < /dev/tty 2>/dev/null || true
                read -r WPAS < /dev/tty || WPAS=""
                stty echo < /dev/tty 2>/dev/null || true
                printf '\n' > /dev/tty
                if $SUDO @networkmanager@/bin/nmcli device wifi connect "$SSID" password "$WPAS"; then
                  log " ? Wi-Fi OK ($SSID)"
                  NET_OK=1
                  save_file wifi-ssid "$SSID"
                  save_file network.conf "connected=1"
                else
                  log " ? FALHA Wi-Fi. Continuar offline? [S/n]"
                  ask_def "> " cont S
                  case "${cont:-S}" in [Nn]*) exit 1 ;; esac
                  save_file network.conf "connected=0"
                fi
              else
                log " ? invalido; offline"
                save_file network.conf "connected=0"
              fi
            fi
            ;;
        esac
      fi
      net_connected && NET_OK=1 || true
      set_step 1
    fi

    # ---- Q2 idioma / voz (uma vez; resume se locale.conf existe) ----
    LANG_SEL="pt_BR.UTF-8"
    KB_DEF=1
    if [ "$cur_step" -ge 2 ] && [ -f /var/lib/nosd/locale.conf ]; then
      LANG_SEL=$(grep -E '^LANG=' /var/lib/nosd/locale.conf | head -1 | cut -d= -f2- | tr -d "\"' " || echo pt_BR.UTF-8)
      log "2) Idioma/voz — resume LANG=$LANG_SEL"
      case "$LANG_SEL" in pt_PT*) KB_DEF=2 ;; en_*) KB_DEF=2 ;; es_*) KB_DEF=3 ;; *) KB_DEF=1 ;; esac
    else
      log ""
      log "2) Idioma do sistema e da voz (Piper/Whisper)"
      log "   1) pt_BR (Brasil)   [default]"
      log "   2) pt_PT (Portugal)"
      log "   3) en_US"
      log "   4) es_ES"
      ask_def "Escolha [1]: " ans 1
      case "${ans:-1}" in
        2) LANG_SEL="pt_PT.UTF-8"; KB_DEF=2 ;;
        3) LANG_SEL="en_US.UTF-8"; KB_DEF=2 ;;
        4) LANG_SEL="es_ES.UTF-8"; KB_DEF=3 ;;
        *) LANG_SEL="pt_BR.UTF-8"; KB_DEF=1 ;;
      esac
      save_file locale.conf "LANG=$LANG_SEL"
      log " ? LANG=$LANG_SEL (voz associada no transmute)"
      set_step 2
    fi

    # ---- Q3 teclado ----
    if [ "$cur_step" -ge 3 ] && [ -f /var/lib/nosd/keyboard.conf ]; then
      KB=$(tr -d ' \n\r' < /var/lib/nosd/keyboard.conf)
      log "3) Teclado — resume $KB"
      case "$KB" in us) KB_LOAD=us ;; es) KB_LOAD=es ;; *) KB=br; KB_LOAD=br-abnt2 ;; esac
      @kbd@/bin/loadkeys "$KB_LOAD" 2>/dev/null || true
    else
      log ""
      log "3) Layout do teclado"
      log "   1) br (ABNT2)"
      log "   2) us"
      log "   3) es"
      ask_def "Escolha [$KB_DEF]: " ans "$KB_DEF"
      case "${ans:-$KB_DEF}" in
        2) KB=us; KB_LOAD=us ;;
        3) KB=es; KB_LOAD=es ;;
        *) KB=br; KB_LOAD=br-abnt2 ;;
      esac
      save_file keyboard.conf "$KB"
      @kbd@/bin/loadkeys "$KB_LOAD" 2>/dev/null || @kbd@/bin/loadkeys "$KB" 2>/dev/null || true
      log " ? teclado=$KB"
      set_step 3
    fi

    # ---- Q4 fuso ----
    if [ "$cur_step" -ge 4 ] && [ -f /var/lib/nosd/timezone ]; then
      TZ=$(tr -d ' \n\r' < /var/lib/nosd/timezone)
      log "4) Fuso — resume $TZ"
    else
      log ""
      log "4) Fuso horario"
      log "   1) America/Sao_Paulo   [default]"
      log "   2) Europe/Lisbon"
      log "   3) UTC"
      log "   4) outro (IANA)"
      ask_def "Escolha [1]: " ans 1
      case "${ans:-1}" in
        2) TZ=Europe/Lisbon ;;
        3) TZ=UTC ;;
        4)
          ask_def "IANA: " TZ America/Sao_Paulo
          [ -n "${TZ:-}" ] || TZ=America/Sao_Paulo
          ;;
        *) TZ=America/Sao_Paulo ;;
      esac
      save_file timezone "$TZ"
      $SUDO @systemd@/bin/timedatectl set-timezone "$TZ" 2>/dev/null || \
        $SUDO ln -sfn "/etc/zoneinfo/$TZ" /etc/localtime 2>/dev/null || true
      log " ? timezone=$TZ"
      set_step 4
    fi

    # ---- Persistência declarativa (prompt Minimal: host-configuration.nix) ----
    # NÃO corre nixos-rebuild switch na ISO live (REJECT triagem). Só grava fragmento.
    KB_NIX="$KB"
    case "$KB" in us) KB_NIX=us ;; es) KB_NIX=es ;; *) KB_NIX=br ;; esac
    HOST_NIX=$(cat <<EOF
# Gerado por nos-firstboot — fragmento para instalacao futura / ai-managed.
# ISO live: NAO aplicar com nixos-rebuild switch automatico.
{ ... }: {
  i18n.defaultLocale = "$LANG_SEL";
  time.timeZone = "$TZ";
  console.keyMap = "$KB_NIX";
  networking.hostName = "nixaios";
}
EOF
)
    printf '%s\n' "$HOST_NIX" | $SUDO tee /var/lib/nosd/host-configuration.nix >/dev/null
    $SUDO mkdir -p /etc/nixos 2>/dev/null || true
    printf '%s\n' "$HOST_NIX" | $SUDO tee /etc/nixos/host-configuration.nix >/dev/null || true
    log "host-configuration.nix gravado em /var/lib/nosd + /etc/nixos (FAT OFF)"

    # ---- Q5 confirmar ----
    log ""
    log "5) Confirmar AIOS Full"
    if [ "$NET_OK" = "1" ] || net_connected; then log "   Rede: OK"; else log "   Rede: offline"; fi
    log "   Idioma/voz: $LANG_SEL"
    log "   Vai baixar MoE + Whisper + Piper (se online) ? Cortex ? Hermes"
    log "   ? JARBAS Slint HUD (cage+iGPU) no tty7 — sem Chromium"
    if [ "$persist_ok" != "1" ]; then
      log "   AVISO: /var/lib/nosd nao gravavel — blobs podem ser volateis."
    fi
    if ! net_connected; then
      log "   AVISO: sem rede — transmute --no-download"
    fi
    ask_def "Prosseguir? [S/n]: " go S
    case "${go:-S}" in
      [Nn]*)
        log "Configs gravadas (passos 1–4). Sair sem transmute."
        $SUDO touch /var/lib/nosd/firstboot.partial
        exit 0
        ;;
    esac
    set_step 5

    log ""
    log "— Garantir nosd —"
    $SUDO systemctl start nosd.service || true
    for i in $(seq 1 40); do
      if $SUDO @nosd@/bin/nosctl --socket /run/nosd/nosd.sock health >/dev/null 2>&1; then
        log "nosd health OK (try=$i)"
        break
      fi
      sleep 0.5
    done
    if ! $SUDO @nosd@/bin/nosctl --socket /run/nosd/nosd.sock health >/dev/null 2>&1; then
      log "FAIL: nosd health offline (socket /run/nosd/nosd.sock)"
      exit 1
    fi

    # Q5 "Prosseguir?" = HITL humano ? passar --confirm (blob_deliberate ModelFetch).
    # Sem --confirm o transmute aborta com await JARBAS HITL e o shell cai no prompt.
    DL_ARGS="--confirm"
    # Lab/headless: nao descarregar GBs sem humano (opt-in explicito).
    if [ "$AUTO" = "1" ] && [ "${NOSD_FIRSTBOOT_DOWNLOAD:-0}" != "1" ]; then
      DL_ARGS="--confirm --no-download"
      log "modo auto: transmute --no-download (NOSD_FIRSTBOOT_DOWNLOAD=1 habilita)"
    fi
    if ! net_connected; then
      DL_ARGS="--confirm --no-download"
      log "— Transmute offline (--confirm --no-download) —"
    else
      log "— Transmute (HITL Q5 + download MoE/Whisper/Piper + hot_load) —"
      log "  pode demorar; journalctl -u nosd -f"
    fi

    # Captura JSON do nosctl para mostrar errors[] no tty (metal pós-Q5).
    TM_OUT=/tmp/nos-transmute-out.json
    TM_RC=0
    $SUDO @nosd@/bin/nosctl --socket /run/nosd/nosd.sock transmute \
        --locale "$LANG_SEL" $DL_ARGS >"$TM_OUT" 2>&1 || TM_RC=$?
    if [ "$TM_RC" != "0" ]; then
      log "AVISO: nosctl transmute exit=$TM_RC — a tentar GUI na mesma (cage)."
      log "  (downloads podem ter falhado; HUD Slint nao depende dos GGUF)"
      if [ -f "$TM_OUT" ]; then
        log "--- transmute response (tail) ---"
        tail -n 40 "$TM_OUT" | while IFS= read -r line; do log "  $line"; done
        # Extrai errors se JSON tiver o campo (python stdlib; sem jq obrigatorio)
        @python3@/bin/python3 -c '
import json,sys
p=sys.argv[1]
try:
  raw=open(p,encoding="utf-8",errors="replace").read()
  # nosctl imprime Response { ok, result/error }; result pode ser TransmuteRunResult
  j=json.loads(raw)
  r=j.get("result") or j
  errs=r.get("errors") if isinstance(r,dict) else None
  note=r.get("note") if isinstance(r,dict) else None
  md=r.get("models_dir") if isinstance(r,dict) else None
  if md: print("models_dir="+str(md))
  if note: print("note="+str(note))
  if errs:
    for e in errs: print("error: "+str(e))
except Exception as ex:
  print("parse_skip: "+str(ex))
' "$TM_OUT" 2>/dev/null | while IFS= read -r line; do log "  $line"; done || true
      fi
      log "  journal: journalctl -u nosd -b --no-pager | tail -40"
    else
      log "transmute RPC OK"
      if [ -f "$TM_OUT" ]; then
        @python3@/bin/python3 -c '
import json,sys
try:
  j=json.loads(open(sys.argv[1],encoding="utf-8",errors="replace").read())
  r=j.get("result") or j
  if isinstance(r,dict):
    if r.get("models_dir"): print("models_dir="+str(r["models_dir"]))
    if r.get("note"): print("note="+str(r["note"]))
    for e in (r.get("errors") or []): print("error: "+str(e))
except Exception:
  pass
' "$TM_OUT" 2>/dev/null | while IFS= read -r line; do log "  $line"; done || true
      fi
    fi

    log "— Hotswap JARBAS Slint (cage tty7 / iGPU) —"
    if [ ! -f /var/lib/nosd/hotswap-aios.request ]; then
      $SUDO touch /var/lib/nosd/hotswap-aios.request
    fi
    $SUDO systemctl reset-failed jarbas-cage.service 2>/dev/null || true
    $SUDO systemctl start nosd-hotswap-aios.service || true
    for i in $(seq 1 90); do
      if systemctl is-active --quiet jarbas-cage.service; then
        log "jarbas-cage ACTIVE (t+$i)"
        break
      fi
      # Se hotswap já falhou, re-tenta cage directo
      if [ "$i" = "15" ] || [ "$i" = "45" ]; then
        log "retry systemctl start jarbas-cage (t+$i)"
        $SUDO systemctl reset-failed jarbas-cage.service 2>/dev/null || true
        $SUDO systemctl start jarbas-cage.service || true
      fi
      sleep 1
    done
    if systemctl is-active --quiet jarbas-cage.service; then
      $SUDO @kbd@/bin/chvt 7 || true
      log "JARBAS Slint HUD no Alt+F7 / tty7 (Enviar = Hermes)."
      log "Logs: /tmp/nos-firstboot.log /var/lib/nosd/{firstboot,hotswap,jarbas-cage}.log"
      $SUDO touch /var/lib/nosd/firstboot.done
      $SUDO chmod a+rw /var/lib/nosd/firstboot.done 2>/dev/null || true
      set_step 9
      log "=== nos-firstboot DONE (GUI up) ==="
    else
      log "FAIL: jarbas-cage NAO active — GUI nao subiu. firstboot.done NAO gravado."
      log "--- status ---"
      systemctl status jarbas-cage.service --no-pager -l || true
      log "--- journal (ultimas 40) ---"
      journalctl -u jarbas-cage -b --no-pager -n 40 || true
      log "--- DRM ---"
      ls -la /dev/dri 2>/dev/null || true
      log "Recuperar:"
      log "  sudo systemctl reset-failed jarbas-cage; sudo systemctl start jarbas-cage; sudo chvt 7"
      log "  # ou: sudo nos-start-gui"
      log "  Logs: /var/lib/nosd/jarbas-cage.log /var/lib/nosd/hotswap.log"
      $SUDO touch /var/lib/nosd/firstboot.gui_failed 2>/dev/null || true
      exit 1
    fi
