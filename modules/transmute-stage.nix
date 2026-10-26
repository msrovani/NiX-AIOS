# Estagio 0: firstboot TEXTO no tty1 (sem GNOME) → nosctl transmute → Hotswap JARBAS.
# s522: GNOME/GIS/Chromium autostart falhou no metal; entrega = questionário bash + cage.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nixaiosTransmute;

  # s529: só Linux FS — FAT NIXAIOSLOG desligada (permissões/metal).
  pickLogDirSh = ''
    LOGDIR=/var/lib/nosd
    mkdir -p "$LOGDIR" 2>/dev/null || true
  '';

  hotswapScript = pkgs.writeShellScript "nosd-hotswap-aios" ''
    set -euo pipefail
    ${pickLogDirSh}
    LOG="$LOGDIR/hotswap.log"
    mkdir -p "$LOGDIR" /var/lib/nosd
    exec > >(tee -a "$LOG") 2>&1
    echo "=== nosd-hotswap-aios start $(date -Is 2>/dev/null || echo now) ==="

    LOC=""
    for f in /var/lib/nosd/locale.conf /etc/locale.conf; do
      if [ -f "$f" ]; then
        cand=$(grep -E '^LANG=' "$f" 2>/dev/null | head -1 | cut -d= -f2- | tr -d "\"' " || true)
        if [ -n "$cand" ]; then LOC="$cand"; break; fi
      fi
    done
    [ -n "$LOC" ] || LOC="pt_BR.UTF-8"
    echo "LANG=$LOC" > /var/lib/nosd/locale.conf
    chown nosd:nosd /var/lib/nosd/locale.conf || true
    chmod a+rw /var/lib/nosd/locale.conf 2>/dev/null || true

    mkdir -p /etc/nixos
    cat > /etc/nixos/ai-managed.nix <<EOF
# Gerado por nos-firstboot / nos-transmute (Hotswap AIOS Full)
{ ... }: {
  i18n.defaultLocale = "$LOC";
  services.nosd.enable = true;
  services.jarbasCage.enable = true;
}
EOF
    echo "hotswap: wrote /etc/nixos/ai-managed.nix (locale=$LOC)"

    echo "hotswap: stopping display-manager (se existir)…"
    systemctl stop display-manager.service 2>/dev/null || true
    pkill -f 'gnome-shell|gdm-wayland-session|Xwayland' 2>/dev/null || true
    sleep 1

    echo "hotswap: restarting nosd…"
    systemctl restart nosd.service || true
    for i in $(seq 1 30); do
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then
        echo "hotswap: nosd HTTP OK"
        break
      fi
      sleep 0.5
    done

    echo "hotswap: starting jarbas-cage.service…"
    systemctl reset-failed jarbas-cage.service seatd.service 2>/dev/null || true
    # Stale sock impede seatd (visto no metal Alienware)
    if [ -S /run/seatd.sock ] && ! systemctl is-active --quiet seatd.service 2>/dev/null; then
      rm -f /run/seatd.sock
    fi
    systemctl start seatd.service 2>/dev/null || true
    if ! systemctl start jarbas-cage.service; then
      echo "hotswap: FAIL systemctl start jarbas-cage" >&2
      journalctl -u jarbas-cage -u seatd -n 40 --no-pager || true
      mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
      exit 1
    fi
    sleep 5
    if ! systemctl is-active --quiet jarbas-cage.service; then
      echo "hotswap: FAIL jarbas-cage not active after start" >&2
      systemctl status jarbas-cage.service --no-pager || true
      journalctl -u jarbas-cage -u seatd -n 40 --no-pager || true
      # Soft: ainda assim chvt + pedir retry manual — firstboot já tenta cage directo
      mv -f /var/lib/nosd/hotswap-aios.request /var/lib/nosd/hotswap-aios.failed 2>/dev/null || true
      exit 1
    fi
    ${pkgs.kbd}/bin/chvt 7 || true
    rm -f /var/lib/nosd/hotswap-aios.request
    echo "=== nosd-hotswap-aios DONE (jarbas-cage active, VT7) ==="
  '';

  revertGuiScript = pkgs.writeShellScript "nosd-revert-gui" ''
    set -euo pipefail
    rm -f /var/lib/nosd/aios-mode 2>/dev/null || true
    systemctl stop jarbas-cage.service 2>/dev/null || true
    echo "revert: jarbas-cage parado; firstboot pode correr de novo (apague firstboot.done)"
  '';

  # Questionário texto: Q1 rede → Q2 idioma/voz → Q3 teclado → Q4 fuso → Q5 confirmar.
  # s523: rede primeiro; resume; log SEMPRE em /tmp (user nixos nao escreve /var/lib/nosd).
  firstbootScript = pkgs.writeShellScriptBin "nos-firstboot" ''
    set -eu
    # NixOS live: pkgs.sudo/bin/sudo NAO tem setuid — so /run/wrappers/bin/sudo
    SUDO=/run/wrappers/bin/sudo
    if [ ! -x "$SUDO" ]; then SUDO=/run/current-system/sw/bin/sudo; fi
    if [ ! -x "$SUDO" ]; then SUDO=sudo; fi
    export PATH="/run/wrappers/bin:/run/current-system/sw/bin:$PATH"
    # s529: FAT OFF — log /tmp + persist /var/lib/nosd (Linux FS)
    LOG=/tmp/nos-firstboot.log
    touch "$LOG" 2>/dev/null || LOG=/tmp/firstboot-$USER.log
    log() {
      echo "$@" | tee -a "$LOG" >/dev/tty 2>/dev/null || echo "$@" > /dev/tty
      echo "$@" | $SUDO tee -a /var/lib/nosd/firstboot.log >/dev/null 2>&1 || true
    }
    ask() { printf '%s' "$1" > /dev/tty; read -r "$2" < /dev/tty || eval "$2="; }

    # Lock em /tmp (nixos nao cria em /run sem perms)
    LOCK=/tmp/nos-firstboot.lock
    exec 9>"$LOCK"
    if ! ${pkgs.util-linux}/bin/flock -n 9; then
      echo "nos-firstboot ja em curso — a sair" > /dev/tty
      exit 0
    fi

    $SUDO mkdir -p /var/lib/nosd /var/lib/nosd/models 2>/dev/null || true
    $SUDO chmod 1777 /var/lib/nosd 2>/dev/null || $SUDO chmod a+rwx /var/lib/nosd 2>/dev/null || true
    $SUDO touch /var/lib/nosd/firstboot.log 2>/dev/null || true
    $SUDO chmod a+rw /var/lib/nosd/firstboot.log 2>/dev/null || true
    echo "firstboot: persist=/var/lib/nosd (FAT NIXAIOSLOG OFF)" > /dev/tty

    log "=== nos-firstboot start $(date -Is 2>/dev/null || echo now) tty=$(tty 2>/dev/null || echo ?) log=$LOG ==="

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
      # $1=name $2=content — so /var/lib/nosd
      printf '%s\n' "$2" | $SUDO tee "/var/lib/nosd/$1" >/dev/null
      $SUDO chmod a+rw "/var/lib/nosd/$1" 2>/dev/null || true
    }
    step_file=/var/lib/nosd/firstboot.step
    set_step() {
      printf '%s\n' "$1" | $SUDO tee "$step_file" >/dev/null
      $SUDO chmod a+rw "$step_file" 2>/dev/null || true
    }
    cur_step=0
    if [ -f "$step_file" ]; then
      cur_step=$(tr -d ' \n\r' < "$step_file" || echo 0)
    fi
    case "$cur_step" in (*[!0-9]*|"") cur_step=0 ;; esac

    net_connected() {
      ${pkgs.networkmanager}/bin/nmcli -t -f STATE g 2>/dev/null | grep -qi '^connected' \
        || ${pkgs.networkmanager}/bin/nmcli -t -f DEVICE,TYPE,STATE device 2>/dev/null | grep -qE ':(ethernet|wifi):connected'
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
        DEV=$(${pkgs.networkmanager}/bin/nmcli -t -f DEVICE,TYPE,STATE device 2>/dev/null | awk -F: '$3=="connected"{print $1" ("$2")"; exit}')
        IP=$(${pkgs.iproute2}/bin/ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}' || true)
        SSID=$(${pkgs.networkmanager}/bin/nmcli -t -f ACTIVE,SSID dev wifi 2>/dev/null | awk -F: '$1=="yes"{print $2; exit}' || true)
        log "   Ja ligado: ''${DEV:-?}''${SSID:+ wifi=$SSID}''${IP:+ src=$IP}"
        NET_OK=1
        [ -n "''${SSID:-}" ] && save_file wifi-ssid "$SSID"
        save_file network.conf "connected=1"
      else
        log "   Sem ligacao."
        log "   1) Wi-Fi (listar SSIDs)"
        log "   2) Esperar cabo Ethernet (30s)"
        log "   3) Continuar offline (sem download HF)"
        ask "Escolha [1]: " ans
        case "''${ans:-1}" in
          2)
            log "   Aguardando Ethernet…"
            for i in $(seq 1 30); do
              if net_connected; then NET_OK=1; log "   Cabo OK (t=$i)"; break; fi
              sleep 1
            done
            if [ "$NET_OK" != "1" ]; then
              log "   Timeout cabo. Continuar offline? [S/n]"
              ask "> " cont
              case "''${cont:-S}" in [Nn]*) log "Abortado."; exit 1 ;; esac
              save_file network.conf "connected=0"
            else
              save_file network.conf "connected=1"
            fi
            ;;
          3)
            log " → offline"
            save_file network.conf "connected=0"
            ;;
          *)
            $SUDO ${pkgs.networkmanager}/bin/nmcli device wifi rescan 2>/dev/null || true
            sleep 2
            mapfile -t WIFI_LIST < <(${pkgs.networkmanager}/bin/nmcli -t -f SSID,SIGNAL,SECURITY device wifi list 2>/dev/null | awk -F: 'NF>=1 && $1!=""{print}' | head -20)
            if [ "''${#WIFI_LIST[@]}" -eq 0 ]; then
              log "   Nenhum SSID. Offline? [S=sim / n=abortar]"
              ask "> " skip
              case "''${skip:-S}" in [Nn]*) exit 1 ;; esac
              save_file network.conf "connected=0"
            else
              i=1
              for line in "''${WIFI_LIST[@]}"; do
                ssid=$(printf '%s' "$line" | cut -d: -f1)
                sig=$(printf '%s' "$line" | cut -d: -f2)
                sec=$(printf '%s' "$line" | cut -d: -f3-)
                log "$(printf "   %2d) %s  (%s%% %s)" "$i" "$ssid" "$sig" "$sec")"
                i=$((i+1))
              done
              log "   S) offline"
              ask "Escolha: " pick
              if [ "$pick" = "S" ] || [ "$pick" = "s" ] || [ -z "''${pick:-}" ]; then
                save_file network.conf "connected=0"
              elif [ "$pick" -ge 1 ] 2>/dev/null && [ "$pick" -le "''${#WIFI_LIST[@]}" ]; then
                SSID=$(printf '%s' "''${WIFI_LIST[$((pick-1))]}" | cut -d: -f1)
                printf "Senha Wi-Fi '%s' (eco oculto): " "$SSID" > /dev/tty
                stty -echo < /dev/tty 2>/dev/null || true
                read -r WPAS < /dev/tty || WPAS=""
                stty echo < /dev/tty 2>/dev/null || true
                printf '\n' > /dev/tty
                if $SUDO ${pkgs.networkmanager}/bin/nmcli device wifi connect "$SSID" password "$WPAS"; then
                  log " → Wi-Fi OK ($SSID)"
                  NET_OK=1
                  save_file wifi-ssid "$SSID"
                  save_file network.conf "connected=1"
                else
                  log " → FALHA Wi-Fi. Continuar offline? [S/n]"
                  ask "> " cont
                  case "''${cont:-S}" in [Nn]*) exit 1 ;; esac
                  save_file network.conf "connected=0"
                fi
              else
                log " → invalido; offline"
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
      ask "Escolha [1]: " ans
      case "''${ans:-1}" in
        2) LANG_SEL="pt_PT.UTF-8"; KB_DEF=2 ;;
        3) LANG_SEL="en_US.UTF-8"; KB_DEF=2 ;;
        4) LANG_SEL="es_ES.UTF-8"; KB_DEF=3 ;;
        *) LANG_SEL="pt_BR.UTF-8"; KB_DEF=1 ;;
      esac
      save_file locale.conf "LANG=$LANG_SEL"
      log " → LANG=$LANG_SEL (voz associada no transmute)"
      set_step 2
    fi

    # ---- Q3 teclado ----
    if [ "$cur_step" -ge 3 ] && [ -f /var/lib/nosd/keyboard.conf ]; then
      KB=$(tr -d ' \n\r' < /var/lib/nosd/keyboard.conf)
      log "3) Teclado — resume $KB"
      case "$KB" in us) KB_LOAD=us ;; es) KB_LOAD=es ;; *) KB=br; KB_LOAD=br-abnt2 ;; esac
      ${pkgs.kbd}/bin/loadkeys "$KB_LOAD" 2>/dev/null || true
    else
      log ""
      log "3) Layout do teclado"
      log "   1) br (ABNT2)"
      log "   2) us"
      log "   3) es"
      ask "Escolha [$KB_DEF]: " ans
      case "''${ans:-$KB_DEF}" in
        2) KB=us; KB_LOAD=us ;;
        3) KB=es; KB_LOAD=es ;;
        *) KB=br; KB_LOAD=br-abnt2 ;;
      esac
      save_file keyboard.conf "$KB"
      ${pkgs.kbd}/bin/loadkeys "$KB_LOAD" 2>/dev/null || ${pkgs.kbd}/bin/loadkeys "$KB" 2>/dev/null || true
      log " → teclado=$KB"
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
      ask "Escolha [1]: " ans
      case "''${ans:-1}" in
        2) TZ=Europe/Lisbon ;;
        3) TZ=UTC ;;
        4)
          ask "IANA: " TZ
          [ -n "''${TZ:-}" ] || TZ=America/Sao_Paulo
          ;;
        *) TZ=America/Sao_Paulo ;;
      esac
      save_file timezone "$TZ"
      $SUDO ${pkgs.systemd}/bin/timedatectl set-timezone "$TZ" 2>/dev/null || \
        $SUDO ln -sfn "/etc/zoneinfo/$TZ" /etc/localtime 2>/dev/null || true
      log " → timezone=$TZ"
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
    log "   Vai baixar MoE + Whisper + Piper (se online) → Cortex → Hermes"
    log "   → JARBAS Slint HUD (cage+iGPU) no tty7 — sem Chromium"
    if [ "$persist_ok" != "1" ]; then
      log "   AVISO: /var/lib/nosd nao gravavel — blobs podem ser volateis."
    fi
    if ! net_connected; then
      log "   AVISO: sem rede — transmute --no-download"
    fi
    ask "Prosseguir? [S/n]: " go
    case "''${go:-S}" in
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
      if ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then
        log "nosd HTTP OK (try=$i)"
        break
      fi
      sleep 0.5
    done
    if ! ${pkgs.curl}/bin/curl -sf -o /dev/null http://127.0.0.1:7701/ 2>/dev/null; then
      log "FAIL: nosd :7701 offline"
      exit 1
    fi

    # Q5 "Prosseguir?" = HITL humano → passar --confirm (blob_deliberate ModelFetch).
    # Sem --confirm o transmute aborta com await JARBAS HITL e o shell cai no prompt.
    DL_ARGS="--confirm"
    if ! net_connected; then
      DL_ARGS="--confirm --no-download"
      log "— Transmute offline (--confirm --no-download) —"
    else
      log "— Transmute (HITL Q5 + download MoE/Whisper/Piper + hot_load) —"
      log "  pode demorar; journalctl -u nosd -f"
    fi

    if ! $SUDO ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute \
        --locale "$LANG_SEL" $DL_ARGS; then
      log "AVISO: nosctl transmute falhou/parcial — a tentar GUI na mesma (cage)."
      log "  (downloads podem ter falhado; HUD Slint nao depende dos GGUF)"
      log "  Log: journalctl -u nosd -b --no-pager | tail -40"
    else
      log "transmute RPC OK"
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
      $SUDO ${pkgs.kbd}/bin/chvt 7 || true
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
  '';

  startGuiScript = pkgs.writeShellScriptBin "nos-start-gui" ''
    set -euo pipefail
    SUDO=/run/wrappers/bin/sudo
    [ -x "$SUDO" ] || SUDO=sudo
    echo "nos-start-gui: reset + start jarbas-cage + chvt 7"
    $SUDO systemctl start nosd.service || true
    $SUDO systemctl reset-failed jarbas-cage.service 2>/dev/null || true
    $SUDO systemctl start nosd-hotswap-aios.service 2>/dev/null || true
    $SUDO systemctl start jarbas-cage.service
    sleep 2
    if systemctl is-active --quiet jarbas-cage.service; then
      $SUDO ${pkgs.kbd}/bin/chvt 7
      echo "OK: cage active — Alt+F7"
      systemctl is-active jarbas-cage.service
    else
      echo "FAIL: cage still down"
      systemctl status jarbas-cage.service --no-pager -l || true
      journalctl -u jarbas-cage -b --no-pager -n 30 || true
      ls -la /dev/dri || true
      exit 1
    fi
  '';

  transmuteCli = pkgs.writeShellScriptBin "nos-transmute" ''
    set -euo pipefail
    CMD="''${1:-run}"
    case "$CMD" in
      gui|firstboot)
        exec ${firstbootScript}/bin/nos-firstboot
        ;;
      status)
        exec ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute-status
        ;;
      run|auto)
        shift || true
        exec ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute "$@"
        ;;
      hotswap)
        exec ${pkgs.nosd}/bin/nosctl --socket /run/nosd/nosd.sock transmute --no-download
        ;;
      revert-gui)
        exec sudo ${revertGuiScript}
        ;;
      *)
        echo "Uso: nos-transmute [firstboot | status | run | hotswap | revert-gui]"
        exit 1
        ;;
    esac
  '';

  # Wrapper com passthru.shellPath (NixOS exige "shell package").
  aiosShellScript = pkgs.writeShellScript "nixos-aios-shell" ''
    {
      echo ""
      echo "**********************************************"
      echo "* Neural AIOS — shell de bootstrap           *"
      echo "**********************************************"
    } > /dev/tty 2>/dev/null || true

    TTY_NOW=$(tty 2>/dev/null || echo unknown)
    echo "[aios] tty=$TTY_NOW" > /dev/tty 2>/dev/null || true

    need_fb=1
    if [ -f /var/lib/nosd/firstboot.done ]; then
      need_fb=0
    fi
    if [ -f /var/lib/nosd/aios-mode ]; then
      mode=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode 2>/dev/null || true)
      if [ "$mode" = "aios" ]; then need_fb=0; fi
    fi

    case "$TTY_NOW" in
      /dev/tty[0-9]*|/dev/console)
        if [ "$need_fb" = "1" ] && [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
          export NOS_FIRSTBOOT_RAN=1
          echo "[aios] a iniciar nos-firstboot…" > /dev/tty 2>/dev/null || true
          ${firstbootScript}/bin/nos-firstboot || echo "[aios] firstboot exit=$?" > /dev/tty 2>/dev/null || true
        fi
        ;;
    esac

    export NOS_FIRSTBOOT_RAN=1
    exec ${pkgs.bashInteractive}/bin/bash -l "$@"
  '';

  loginShell = pkgs.runCommand "nixos-aios-shell" {
    passthru = { shellPath = "/bin/nixos-aios-shell"; };
  } ''
    mkdir -p $out/bin
    cp ${aiosShellScript} $out/bin/nixos-aios-shell
    chmod 755 $out/bin/nixos-aios-shell
  '';
in
{
  options.services.nixaiosTransmute = {
    enable = lib.mkEnableOption "Firstboot texto tty1 + transmute + Hotswap JARBAS";
  };

  config = lib.mkIf cfg.enable {
    time.timeZone = lib.mkDefault "America/Sao_Paulo";
    i18n.defaultLocale = lib.mkDefault "pt_BR.UTF-8";
    i18n.supportedLocales = lib.mkDefault [
      "pt_BR.UTF-8/UTF-8"
      "pt_PT.UTF-8/UTF-8"
      "en_US.UTF-8/UTF-8"
      "es_ES.UTF-8/UTF-8"
    ];
    console.keyMap = lib.mkDefault "br";

    # SEM GNOME / GDM — console only ate hotswap.
    services.xserver.enable = lib.mkForce false;
    services.displayManager.gdm.enable = lib.mkForce false;
    services.desktopManager.gnome.enable = lib.mkForce false;
    services.xserver.displayManager.gdm.enable = lib.mkForce false;
    services.xserver.desktopManager.gnome.enable = lib.mkForce false;
    services.displayManager.autoLogin.enable = lib.mkForce false;

    services.getty.autologinUser = lib.mkForce "nixos";

    # Shell wrapper = disparo fiavel (nao depende de profile.d)
    users.users.nixos.shell = lib.mkForce loginShell;

    # Backup 1: loginShellInit
    environment.loginShellInit = lib.mkAfter ''
      if [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
        case "$(tty 2>/dev/null)" in
          /dev/tty[0-9]*|/dev/console)
            if [ ! -f /var/lib/nosd/firstboot.done ]; then
              export NOS_FIRSTBOOT_RAN=1
              ${firstbootScript}/bin/nos-firstboot || true
            fi
            ;;
        esac
      fi
    '';

    # Backup 2: .bash_profile no home (sobrevive se shell voltar a bash)
    system.activationScripts.nosFirstbootBashProfile = {
      text = ''
        mkdir -p /home/nixos
        cat > /home/nixos/.bash_profile << 'EOF'
# Neural AIOS firstboot (s524) — forced
echo "[aios] .bash_profile tty=$(tty 2>/dev/null)"
if [ -z "''${NOS_FIRSTBOOT_RAN:-}" ]; then
  case "$(tty 2>/dev/null)" in
    /dev/tty*|/dev/console)
      if [ ! -f /var/lib/nosd/firstboot.done ]; then
        export NOS_FIRSTBOOT_RAN=1
        nos-firstboot || true
      fi
      ;;
  esac
fi
[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"
EOF
        chown nixos:users /home/nixos/.bash_profile 2>/dev/null || true
        chmod 644 /home/nixos/.bash_profile 2>/dev/null || true
      '';
      deps = [ "users" ];
    };

    environment.etc.motd.text = ''

      Neural AIOS — questionario: nos-firstboot
      GUI (tty7):            nos-start-gui
      Se ja fez Q1–Q5:       sudo systemctl start jarbas-cage && sudo chvt 7

    '';

    security.sudo.enable = true;
    security.sudo.wheelNeedsPassword = false;
    # Garante wrapper setuid em /run/wrappers/bin/sudo (nao usar pkgs.sudo/bin)
    services.jarbasCage.autoStart = lib.mkForce false;

    environment.systemPackages = with pkgs; [
      firstbootScript
      transmuteCli
      startGuiScript
      loginShell
      networkmanager
      curl
      kbd
      llama-cpp
      whisper-cpp
      espeak-ng
      alsa-utils
    ];

    systemd.paths.nosd-hotswap-aios = {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = "/var/lib/nosd/hotswap-aios.request";
        Unit = "nosd-hotswap-aios.service";
      };
    };

    systemd.services.nosd-hotswap-aios = {
      description = "Hotswap texto/bootstrap → JARBAS AIOS Kiosk";
      after = [ "nosd.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${hotswapScript}";
      };
    };

    # s529: sem FAT — resume via /var/lib/nosd/aios-mode (overlay Linux)
    systemd.services.nosd-boot-persist-check = {
      description = "Auto-enter JARBAS if /var/lib/nosd already transmuted";
      wantedBy = [ "multi-user.target" ];
      after = [ "nosd.service" "getty@tty1.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "nosd-boot-persist-check" ''
          sleep 3
          if [ -f /var/lib/nosd/aios-mode ]; then
            mode=$(tr -d ' \n\r' < /var/lib/nosd/aios-mode || true)
            if [ "$mode" = "aios" ]; then
              touch /var/lib/nosd/firstboot.done
              touch /var/lib/nosd/hotswap-aios.request
              systemctl start --no-block nosd-hotswap-aios.service || true
            fi
          fi
        '';
      };
    };
  };
}
