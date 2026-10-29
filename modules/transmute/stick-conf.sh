# Helpers de persistencia no pendrive NIXAIOSLOG (source pelo nos-firstboot).
# Requer: STICK_DIR, STICK_CFG, STICK_OK, SUDO (pode ser vazio em teste).
# Formato: linhas KEY=VALUE; nunca fazer `source` do ficheiro do pendrive.

stick_get() {
  [ "$STICK_OK" = "1" ] && [ -f "$STICK_CFG" ] || return 1
  local line
  line=$(grep -m1 "^$1=" "$STICK_CFG" 2>/dev/null) || return 1
  printf '%s\n' "${line#*=}"
}

save_stick() {
  # $1=ficheiro $2=conteudo; sem FAT nao faz nada.
  [ "$STICK_OK" = "1" ] || return 0
  printf '%s\n' "$2" | ${SUDO:-} tee "$STICK_DIR/$1" >/dev/null 2>&1 || true
  sync 2>/dev/null || true
}

save_stick_cfg() {
  save_stick firstboot.conf "DONE=${1:-0}
LANG=${LANG_SEL:-pt_BR.UTF-8}
KEYMAP=${KB:-br}
TZ=${TZ:-America/Sao_Paulo}"
}
