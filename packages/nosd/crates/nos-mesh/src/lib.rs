//! Mesh P2P — contratos wire ADR-0081 (PORT sobre UDP std).

pub mod crdt;
pub mod fleet_health;
pub mod frack;
pub mod frag;
pub mod ident;
pub mod job;
pub mod laya_intent;
pub mod mesh_knowledge;
pub mod noise;
pub mod soul_mirror;
pub mod udp;

pub use crdt::{CrdtEntry, CrdtStore, VersionVector};
pub use fleet_health::{
    encode_mch, fleet_worst, parse_mch, HealthVerdict, MachineHealth, MAGIC_MCH,
};
pub use frack::Frack;
pub use frag::{FragHeader, FragReassembler};
pub use ident::{Identity, PinError, Pinned};
pub use laya_intent::{LayaIntent, WIRE_VERSION as LAYA_WIRE_VERSION};
pub use mesh_knowledge::{content_fingerprint, fnv1a64, payload_fingerprint};
pub use soul_mirror::SoulMirrorSnapshot;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub const MAGIC_HB: &[u8] = b"HB\0";

/// S1: commitment de conte?do (BLAKE3 hex, 64 chars).
pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
pub const MAGIC_FRAG: &[u8] = frag::MAGIC;
pub const MAGIC_ROLE: &[u8] = b"ROLE\0";
/// Janela mínima após 1º peer PK-bound antes de ROLE / SkillSync (ADR-0081).
pub const TOFU_SETTLE_TICKS: u32 = 130;
/// Equivalente ~130 ticks @ ~18 Hz quando `tick()` não corre.
pub const TOFU_SETTLE_MS: u64 = 7_200;
/// M8.1c: acima disto o log ? compactado no arranque (1 linha por entrada).
pub const CRDT_COMPACT_LINES: usize = 64;
/// M8.1d: a ancora anti-rollback e actualizada a cada N linhas escritas.
pub const CRDT_ANCHOR_EVERY: usize = 32;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerRole {
    Master,
    Worker,
    Memory,
    Compute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapacityScore {
    pub cpu_units: u32,
    pub mem_mib: u32,
    pub net_score: u32,
}

impl CapacityScore {
    pub fn total(&self) -> u64 {
        u64::from(self.cpu_units) * 10_000 + u64::from(self.mem_mib) + u64::from(self.net_score)
    }
}

impl Default for CapacityScore {
    fn default() -> Self {
        Self {
            cpu_units: 4,
            mem_mib: 2048,
            net_score: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: u32,
    pub last_seen_ms: u64,
    pub pk_bound: bool,
    /// Quando `pk_bound` ficou true (ms epoch) — início da janela TOFU.
    pub pk_bound_at_ms: Option<u64>,
    pub capacity: CapacityScore,
    pub role: PeerRole,
}

/// M8.1d: formato em disco do log do CRDT.
///
/// - `Jsonl`: `data_dir/crdt.jsonl` — linhas `json\tbase64(sig)`, legado.
/// - `Nsgdb`: `data_dir/nsgdb/crdt/seg.bin` — segmento **length-prefixed**
///   (`NSGC` + versao + `u32 len` + payload). Segue a honestidade do backend
///   de memoria: e um segmento local, **nao** o motor neural-sgdb completo
///   (sem CRDT/BQ/semantico do lado do motor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrdtLogFormat {
    Jsonl,
    Nsgdb,
}

impl CrdtLogFormat {
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "nsgdb" => Self::Nsgdb,
            _ => Self::Jsonl,
        }
    }
}

/// M8.1d: magic do segmento CRDT (distinto de `NSGS` da memoria).
pub const CRDT_SEG_MAGIC: [u8; 4] = *b"NSGC";
pub const CRDT_SEG_VERSION: u32 = 1;
/// Cap de registo: evita `vec![0; 4GiB]` com um comprimento corrompido.
pub const CRDT_MAX_RECORD_BYTES: u32 = 16 * 1024 * 1024;

/// T1.1: mensagem assinada da ancora (distingue de outros registos Ed25519).
fn anchor_msg(bytes: u64, lines: u64) -> String {
    format!("NSGANCHOR {bytes} {lines}")
}

/// M8.1c: log do CRDT com append barato e compacta??o (runtime).
pub struct CrdtLog {
    path: std::path::PathBuf,
    file: Mutex<std::fs::File>,
    lines: std::sync::atomic::AtomicUsize,
    /// M8.1e: identidade para assinar cada linha (integridade em repouso).
    signer: Option<ident::Identity>,
    /// M8.1d: ancora anti-rollback (`<bytes> <linhas>` ja vistos em disco).
    anchor: std::path::PathBuf,
    /// M8.1d: `jsonl` (linhas) ou `nsgdb` (segmento length-prefixed).
    fmt: CrdtLogFormat,
}

impl CrdtLog {
    /// M8.1d: grava a ancora (`<bytes> <linhas>`) de forma atomica.
    ///
    /// Antes de gravar, faz `fsync` do log aberto: a ancora nunca pode ficar
    /// "a frente" dos dados (num crash isso seria um falso rollback).
    fn write_anchor(&self) -> std::io::Result<()> {
        let _ = self.file.lock().sync_all();
        let len = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        let lines = self.lines.load(std::sync::atomic::Ordering::Relaxed) as u64;
        self.write_anchor_at(len, lines)
    }

    /// Publica a ancora de um log que ainda vai ser posto no lugar
    /// (compactacao: o log novo e sempre menor, logo anuncia-lo antes do
    /// `rename` evita uma janela em que a ancora antiga acusaria rollback).
    fn write_anchor_at(&self, bytes: u64, lines: u64) -> std::io::Result<()> {
        let tmp = self.anchor.with_extension("anchor.tmp");
        // T1.1: ancora assinada pela identidade do no (`<bytes> <linhas>` +
        // `sig b64` sobre `NSGANCHOR bytes lines`). Sem assinatura, quem
        // escreve no data-dir repõe log+ancora em conjunto e o rollback fica
        // indistinguível de um estado antigo legítimo.
        match self.signer.as_ref() {
            Some(id) => {
                use base64::Engine as _;
                let sig = base64::engine::general_purpose::STANDARD
                    .encode(id.sign(anchor_msg(bytes, lines).as_bytes()));
                std::fs::write(&tmp, format!("{bytes} {lines} {sig}"))?;
            }
            None => std::fs::write(&tmp, format!("{bytes} {lines}"))?,
        }
        std::fs::rename(&tmp, &self.anchor)?;
        if let Some(dir) = self.anchor.parent() {
            if let Ok(d) = std::fs::File::open(dir) {
                let _ = d.sync_all();
            }
        }
        Ok(())
    }
}

/// Selo de um registo: `json` ou `json \t base64(sig(json))`, sem delimitador.
fn seal_record(e: &CrdtEntry, id: Option<&ident::Identity>) -> Option<Vec<u8>> {
    let json = serde_json::to_vec(e).ok()?;
    match id {
        None => Some(json),
        Some(id) => {
            use base64::Engine as _;
            let sig = base64::engine::general_purpose::STANDARD.encode(id.sign(&json));
            let mut out = json;
            out.push(b'\t');
            out.extend_from_slice(sig.as_bytes());
            Some(out)
        }
    }
}

/// Abre um registo. `Err(())` = recusado (assinatura inv?lida/ausente).
fn open_record(rec: &[u8], id: Option<&ident::Identity>) -> Result<Option<CrdtEntry>, ()> {
    use base64::Engine as _;
    if rec.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(None);
    }
    let (json, sig) = match rec.iter().position(|b| *b == b'\t') {
        Some(i) => (&rec[..i], Some(&rec[i + 1..])),
        None => (rec, None),
    };
    if let Some(id) = id {
        let Some(sig) = sig else {
            // Com identidade, registo sem assinatura ? recusado (fail-closed).
            return Err(());
        };
        let Ok(sig) = std::str::from_utf8(sig) else {
            return Err(());
        };
        let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(sig) else {
            return Err(());
        };
        if raw.len() != 64 || !crate::ident::verify(&id.public_key(), json, &raw) {
            return Err(());
        }
    }
    serde_json::from_slice(json).map(Some).map_err(|_| ())
}

/// Le o log inteiro: `(bytes no disco, registos)`.
///
/// Um tail truncado (crash a meio de um write) e ignorado, tal como uma linha
/// parcial no formato jsonl.
fn read_log(path: &std::path::Path, fmt: CrdtLogFormat) -> std::io::Result<(u64, Vec<Vec<u8>>)> {
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, Vec::new())),
        Err(e) => return Err(e),
    };
    let bytes = raw.len() as u64;
    match fmt {
        CrdtLogFormat::Jsonl => {
            let mut out = Vec::new();
            for line in raw.split(|b| *b == b'\n') {
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                if line.iter().all(|b| b.is_ascii_whitespace()) {
                    continue;
                }
                out.push(line.to_vec());
            }
            Ok((bytes, out))
        }
        CrdtLogFormat::Nsgdb => {
            // Cabecalho incompleto = ficheiro novo a meio de um create: vazio.
            if raw.len() < 8 {
                return Ok((bytes, Vec::new()));
            }
            if raw[..4] != CRDT_SEG_MAGIC {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "crdt seg: magic invalido (esperado NSGC)",
                ));
            }
            let ver = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
            if ver != CRDT_SEG_VERSION {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("crdt seg: versao {ver} nao suportada"),
                ));
            }
            let mut out = Vec::new();
            let mut off = 8usize;
            while off + 4 <= raw.len() {
                let len = u32::from_le_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]])
                    as usize;
                if len > CRDT_MAX_RECORD_BYTES as usize {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "crdt seg: registo acima do cap",
                    ));
                }
                if off + 4 + len > raw.len() {
                    break; // tail parcial (crash)
                }
                if len > 0 {
                    out.push(raw[off + 4..off + 4 + len].to_vec());
                }
                off += 4 + len;
            }
            Ok((bytes, out))
        }
    }
}

impl CrdtLog {
    fn append(&self, entries: &[CrdtEntry]) {
        use std::io::Write;
        let mut n = 0usize;
        {
            let mut g = self.file.lock();
            for e in entries {
                let Some(rec) = seal_record(e, self.signer.as_ref()) else {
                    continue;
                };
                let ok = match self.fmt {
                    CrdtLogFormat::Jsonl => {
                        let mut line = rec;
                        line.push(b'\n');
                        g.write_all(&line).is_ok()
                    }
                    CrdtLogFormat::Nsgdb => match u32::try_from(rec.len()) {
                        Ok(len) if len <= CRDT_MAX_RECORD_BYTES && len > 0 => g
                            .write_all(&len.to_le_bytes())
                            .and_then(|_| g.write_all(&rec))
                            .is_ok(),
                        _ => false,
                    },
                };
                if ok {
                    n += 1;
                }
            }
            let _ = g.flush();
        }
        let prev = self
            .lines
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
        // M8.1d: checkpoint periodico (limita a janela de rollback indetectado).
        if n > 0 && prev / CRDT_ANCHOR_EVERY != (prev + n) / CRDT_ANCHOR_EVERY {
            let _ = self.write_anchor();
        }
    }

    fn compact(&self, store: &CrdtStore) -> std::io::Result<()> {
        use std::io::Write;
        let tmp = match self.fmt {
            CrdtLogFormat::Jsonl => self.path.with_extension("jsonl.tmp"),
            CrdtLogFormat::Nsgdb => self.path.with_extension("bin.tmp"),
        };
        {
            let mut f = std::fs::File::create(&tmp)?;
            if self.fmt == CrdtLogFormat::Nsgdb {
                f.write_all(&CRDT_SEG_MAGIC)?;
                f.write_all(&CRDT_SEG_VERSION.to_le_bytes())?;
            }
            for e in store.entries.values() {
                let Some(rec) = seal_record(e, self.signer.as_ref()) else {
                    continue;
                };
                match self.fmt {
                    CrdtLogFormat::Jsonl => {
                        f.write_all(&rec)?;
                        f.write_all(b"\n")?;
                    }
                    CrdtLogFormat::Nsgdb => {
                        let len = u32::try_from(rec.len()).map_err(|_| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "crdt seg: registo acima de 4 GiB",
                            )
                        })?;
                        f.write_all(&len.to_le_bytes())?;
                        f.write_all(&rec)?;
                    }
                }
            }
            f.sync_all()?;
        }
        let new_len = std::fs::metadata(&tmp)?.len();
        // M8.1d: publica a ancora do novo log antes do rename. Uma compactacao
        // so encolhe (entradas <= linhas), por isso anunciar primeiro e seguro e
        // elimina a janela de crash em que a ancora antiga acusaria rollback.
        self.write_anchor_at(new_len, store.entries.len() as u64)?;
        std::fs::rename(&tmp, &self.path)?;
        // fsync do diret?rio: garante que o rename sobrevive ao crash.
        if let Some(dir) = self.path.parent() {
            if let Ok(d) = std::fs::File::open(dir) {
                let _ = d.sync_all();
            }
        }
        *self.file.lock() = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.lines
            .store(store.entries.len(), std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[derive(Clone)]
pub struct MeshEngine {
    peers: Arc<Mutex<HashMap<u32, PeerInfo>>>,
    local_id: u32,
    local_capacity: CapacityScore,
    local_role: Arc<Mutex<PeerRole>>,
    tofu_ticks: Arc<Mutex<u32>>,
    first_pk_bound_ms: Arc<Mutex<Option<u64>>>,
    frags: Arc<FragReassembler>,
    /// Último payload FRAG reassembled (lab/IPC).
    last_frag: Arc<Mutex<Option<Vec<u8>>>>,
    /// ACKs a enviar após receber FRAG (stop-and-wait lab).
    pending_fracks: Arc<Mutex<Vec<Frack>>>,
    fracks_received: Arc<Mutex<Vec<Frack>>>,
    /// Saúde local para FLEET worst-of (HUD / RPC).
    local_fleet: Arc<Mutex<Option<MachineHealth>>>,
    /// Saúde reportada por peers (MCH wire).
    peer_fleet: Arc<Mutex<HashMap<u32, MachineHealth>>>,
    /// Último LayaIntent recebido ou emitido (lab).
    last_laya: Arc<Mutex<Option<LayaIntent>>>,
    /// CRDT thin local (SoulMirror / M.2) — sync P2P residual.
    crdt: Arc<Mutex<CrdtStore>>,
    /// M6: identidade Ed25519 (None = HB sem assinatura, compat lab).
    ident: Option<ident::Identity>,
    /// M6: pinagem TOFU + anti-replay (seq monotónico por peer).
    pinned: Arc<ident::Pinned>,
    /// M6: contador de HB local.
    hb_seq: Arc<std::sync::atomic::AtomicU64>,
    /// T1.3: ficheiro com o último seq emitido (restart arranca de
    /// max(epoch_ms, persistido+1); fecha a janela de Replay com relógio a
    /// recuar entre restarts).
    hb_seq_store: Option<std::path::PathBuf>,
    /// M8.1b/c: log append-only + compacta??o (JSONL).
    crdt_log: Option<Arc<CrdtLog>>,
}

impl Default for MeshEngine {
    fn default() -> Self {
        Self::new(15)
    }
}

struct SignedHb<'a> {
    id: u32,
    seq: u64,
    pk: &'a [u8],
    msg: &'a [u8],
    sig: &'a [u8],
    cap: Option<CapacityScore>,
}

impl MeshEngine {
    pub fn new(local_id: u32) -> Self {
        Self::with_capacity(local_id, CapacityScore::default())
    }

    pub fn with_capacity(local_id: u32, capacity: CapacityScore) -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            local_id,
            local_capacity: capacity,
            local_role: Arc::new(Mutex::new(PeerRole::Worker)),
            tofu_ticks: Arc::new(Mutex::new(0)),
            first_pk_bound_ms: Arc::new(Mutex::new(None)),
            frags: Arc::new(FragReassembler::new()),
            last_frag: Arc::new(Mutex::new(None)),
            pending_fracks: Arc::new(Mutex::new(Vec::new())),
            fracks_received: Arc::new(Mutex::new(Vec::new())),
            local_fleet: Arc::new(Mutex::new(None)),
            peer_fleet: Arc::new(Mutex::new(HashMap::new())),
            last_laya: Arc::new(Mutex::new(None)),
            crdt: Arc::new(Mutex::new(CrdtStore::new())),
            ident: None,
            pinned: Arc::new(ident::Pinned::new()),
            // M5.1: o seq tem de sobreviver a um restart do processo. Um
            // contador a começar em 1 fazia os peers rejeitarem o HB do no
            // reiniciado como Replay para sempre. Base = epoch ms (segue
            // monotonicamente enquanto o relogio nao andar para tras).
            hb_seq: Arc::new(std::sync::atomic::AtomicU64::new(now_ms().max(1))),
            hb_seq_store: None,
            crdt_log: None,
        }
    }

    /// M6: mesh com identidade; HB passa a ser assinado e verificado.
    /// O `local_id` passa a ser **derivado da chave** (o `--node-id` deixa de
    /// mandar quando ha identidade): id escolhido pelo operador nao e identidade.
    pub fn with_identity(
        _local_id: u32,
        capacity: CapacityScore,
        identity: ident::Identity,
    ) -> Self {
        let derived = identity.node_id();
        let mut m = Self::with_capacity(derived, capacity);
        m.ident = Some(identity);
        m
    }

    /// T1.2: substitui os pins por um store persistido no disco (a mesma
    /// confiança fica partilhada entre o HB UDP e o listener de jobs).
    pub fn with_pinned_store(mut self, path: &std::path::Path) -> std::io::Result<Self> {
        self.pinned = Arc::new(ident::Pinned::load_or_create(path)?);
        Ok(self)
    }

    /// T1.3: ativa a persistência do seq do HB. O seq de arranque passa a
    /// `max(epoch_ms, persistido+1)`.
    pub fn with_hb_seq_store(mut self, path: &std::path::Path) -> std::io::Result<Self> {
        let stored = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let now = now_ms().max(1);
        let floor = now.max(stored.saturating_add(1));
        self.hb_seq
            .store(floor, std::sync::atomic::Ordering::Relaxed);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // T1.3: o piso (floor) fica em disco já no arranque — assim um restart
        // imediato (sem HB emitido) ainda arranca à frente do seq já visto
        // pelos peers.
        let tmp = path.with_extension("seq.tmp");
        let _ = std::fs::write(&tmp, floor.to_string());
        let _ = std::fs::rename(&tmp, path);
        self.hb_seq_store = Some(path.to_path_buf());
        Ok(self)
    }

    /// T1.3: leitura do seq actual (testes/diagnóstico).
    pub fn hb_seq(&self) -> u64 {
        self.hb_seq.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// T1.3: persiste o seq emitido (a cada 16 HBs; o custo de um crash com
    /// pins ~16 HBs atrasados e rejeitar replay ate o relogio alcancar).
    fn persist_hb_seq(&self, seq: u64) {
        if !seq.is_multiple_of(16) {
            return;
        }
        let Some(ref path) = self.hb_seq_store else {
            return;
        };
        let tmp = path.with_extension("seq.tmp");
        if std::fs::write(&tmp, seq.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    /// M8.1b/c: replay do log (jsonl), compacta??o (snapshot + truncate) e
    /// append. Mantem o formato legado; para o segmento nsgdb usar
    /// [`Self::with_crdt_log_fmt`].
    pub fn with_crdt_log(self, path: &std::path::Path) -> std::io::Result<Self> {
        self.with_crdt_log_fmt(path, CrdtLogFormat::Jsonl)
    }

    /// M8.1b/c/d: replay, compacta??o e append num formato explicito.
    ///
    /// - Registos inv?lidos (crash a meio de um `write`) s?o ignorados.
    /// - Acima de `CRDT_COMPACT_LINES`, o log ? reescrito com um registo por
    ///   entrada antes de reabrir (evita replay O(ops) para sempre).
    /// - `Nsgdb` grava um segmento `NSGC` length-prefixed em vez de JSONL.
    pub fn with_crdt_log_fmt(
        mut self,
        path: &std::path::Path,
        fmt: CrdtLogFormat,
    ) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let anchor = path.with_extension("anchor");
        let (file_bytes, records) = read_log(path, fmt)?;
        let file_lines = records.len() as u64;
        // M8.1d: rollback = log mais pequeno (bytes ou linhas) do que a ancora.
        // A ancora e actualizada ao abrir, ao compactar e a cada
        // CRDT_ANCHOR_EVERY linhas; um log que so cresce nunca acusa.
        match std::fs::read_to_string(&anchor) {
            Ok(a) => {
                let parse_u64 = |s: &str| {
                    s.parse::<u64>().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "ancora crdt ilegivel: {} (apague-a para aceitar o estado actual)",
                                anchor.display()
                            ),
                        )
                    })
                };
                let fields: Vec<&str> = a.split_whitespace().collect();
                let (prev_bytes, prev_lines) = match fields.len() {
                    // T1.1: com identidade, ancora sem assinatura e recusada —
                    // quem a substituiu pode estar a esconder um rollback.
                    // Escapataria honesta (igual aos outros erros de ancora):
                    // apagar a ancora aceita o estado actual do log.
                    2 if self.ident.is_some() => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "ancora crdt sem assinatura com identidade activa: {} \
                                 (apague-a para aceitar o estado actual)",
                                anchor.display()
                            ),
                        ));
                    }
                    2 => (parse_u64(fields[0])?, parse_u64(fields[1])?),
                    // T1.1: ancora assinada (`<bytes> <linhas> <sig b64>`).
                    3 => {
                        let prev = (parse_u64(fields[0])?, parse_u64(fields[1])?);
                        if let Some(id) = self.ident.as_ref() {
                            use base64::Engine as _;
                            let ok = base64::engine::general_purpose::STANDARD
                                .decode(fields[2])
                                .ok()
                                .filter(|raw| {
                                    raw.len() == 64
                                        && crate::ident::verify(
                                            &id.public_key(),
                                            anchor_msg(prev.0, prev.1).as_bytes(),
                                            raw,
                                        )
                                })
                                .is_some();
                            if !ok {
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!(
                                        "ancora crdt com assinatura invalida: {} (apague-a \
                                         para aceitar o estado actual)",
                                        anchor.display()
                                    ),
                                ));
                            }
                        }
                        prev
                    }
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "ancora crdt incompleta: {} (apague-a para aceitar o estado actual)",
                                anchor.display()
                            ),
                        ));
                    }
                };
                if file_bytes < prev_bytes || file_lines < prev_lines {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "crdt rollback suspeito: log {file_bytes} bytes/{file_lines} \
                             linhas < ancora {prev_bytes}/{prev_lines} (apague {})",
                            anchor.display()
                        ),
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut store = self.crdt.lock();
        let mut lines = 0usize;
        let mut rejected = 0usize;
        let signer = self.ident.clone();
        for rec in &records {
            lines += 1;
            match open_record(rec, signer.as_ref()) {
                Ok(Some(e)) => store.merge_entry(e),
                Ok(None) => {}
                Err(()) => rejected += 1,
            }
        }
        if rejected > 0 {
            warn!(
                target: "nosd",
                "crdt log: {rejected} linha(s) recusada(s) (assinatura/JSON)"
            );
        }
        let replayed = store.entries.len();
        drop(store);
        if fmt == CrdtLogFormat::Nsgdb && file_bytes < 8 {
            // Ficheiro novo (ou cabecalho incompleto): cria o segmento.
            use std::io::Write;
            let mut f = std::fs::File::create(path)?;
            f.write_all(&CRDT_SEG_MAGIC)?;
            f.write_all(&CRDT_SEG_VERSION.to_le_bytes())?;
            f.sync_all()?;
        }
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let log = Arc::new(CrdtLog {
            path: path.to_path_buf(),
            file: Mutex::new(f),
            lines: std::sync::atomic::AtomicUsize::new(0),
            signer,
            anchor,
            fmt,
        });
        let store = self.crdt.lock().clone();
        if lines > CRDT_COMPACT_LINES {
            log.compact(&store)?;
        } else {
            log.lines.store(lines, std::sync::atomic::Ordering::Relaxed);
        }
        // M8.1d: a ancora passa a reflectir o estado em disco (replay/compact).
        let _ = log.write_anchor();
        self.crdt_log = Some(log);
        info!(
            target: "nosd",
            "crdt log {:?} fmt={:?} replayed={replayed}",
            path,
            fmt
        );
        Ok(self)
    }

    fn log_entries(&self, entries: &[CrdtEntry]) {
        if let Some(ref log) = self.crdt_log {
            log.append(entries);
        }
    }

    /// M8.1c: compacta em runtime se o log passou do limite (chamada peri?dica).
    pub fn crdt_maybe_compact(&self) -> std::io::Result<bool> {
        let Some(ref log) = self.crdt_log else {
            return Ok(false);
        };
        if log.lines.load(std::sync::atomic::Ordering::Relaxed) <= CRDT_COMPACT_LINES {
            return Ok(false);
        }
        let store = self.crdt.lock().clone();
        log.compact(&store)?;
        Ok(true)
    }

    /// M8.1d: reescreve o log a partir do estado em memoria (compacta??o
    /// for?ada). Usado pela migra??o jsonl -> nsgdb: o motor j? tem o estado
    /// replicado e o log novo nasce com uma entrada por chave.
    pub fn crdt_rewrite_log(&self) -> std::io::Result<()> {
        let Some(ref log) = self.crdt_log else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "sem log crdt para reescrever",
            ));
        };
        let store = self.crdt.lock().clone();
        log.compact(&store)
    }

    pub fn identity_id(&self) -> Option<u32> {
        self.ident.as_ref().map(|i| i.node_id())
    }

    pub fn pinned_peers(&self) -> usize {
        self.pinned.peer_count()
    }

    /// M7: store de chaves partilhado com o listener de jobs (mesma confiança).
    pub fn pinned(&self) -> Arc<ident::Pinned> {
        self.pinned.clone()
    }

    pub fn identity(&self) -> Option<ident::Identity> {
        self.ident.clone()
    }

    pub fn local_id(&self) -> u32 {
        self.local_id
    }

    pub fn crdt_put_soul(&self, snap: &SoulMirrorSnapshot) {
        let mut store = self.crdt.lock();
        snap.into_crdt(&mut store);
    }

    pub fn crdt_get(&self, key: &str) -> Option<CrdtEntry> {
        self.crdt.lock().get(key).cloned()
    }

    /// M8: put local (HUD/estado). `peer` entra no version vector.
    pub fn crdt_put(&self, peer: u32, key: &str, value: &str, tick: u64) {
        {
            let mut g = self.crdt.lock();
            g.put(peer, key, value, tick);
        }
        if let Some(e) = self.crdt_get(key) {
            self.log_entries(&[e]);
        }
    }

    /// M8: snapshot para anti-entropy (estado completo nesta fatia).
    pub fn crdt_snapshot(&self) -> CrdtStore {
        self.crdt.lock().clone()
    }

    /// M8: merge de um store remoto (idempotente, LWW por VV).
    pub fn crdt_merge(&self, other: &CrdtStore) {
        self.crdt.lock().merge_store(other);
    }

    pub fn crdt_len(&self) -> usize {
        self.crdt.lock().entries.len()
    }

    /// M8.1: VV local e delta desde um VV remoto.
    pub fn crdt_version(&self) -> crate::crdt::VersionVector {
        self.crdt.lock().version_vector()
    }

    pub fn crdt_delta_since(&self, since: &crate::crdt::VersionVector) -> Vec<CrdtEntry> {
        self.crdt.lock().delta_since(since)
    }

    pub fn crdt_merge_entries(&self, entries: &[CrdtEntry]) {
        if entries.is_empty() {
            return;
        }
        {
            let mut g = self.crdt.lock();
            for e in entries {
                g.merge_entry(e.clone());
            }
        }
        self.log_entries(entries);
    }

    pub fn on_heartbeat(&self, peer_id: u32, has_pk: bool) {
        self.on_heartbeat_inner(peer_id, has_pk, None);
    }

    pub fn on_heartbeat_with_capacity(&self, peer_id: u32, has_pk: bool, capacity: CapacityScore) {
        self.on_heartbeat_inner(peer_id, has_pk, Some(capacity));
    }

    fn on_heartbeat_inner(&self, peer_id: u32, has_pk: bool, capacity: Option<CapacityScore>) {
        let now = now_ms();
        let mut g = self.peers.lock();
        let e = g.entry(peer_id).or_insert(PeerInfo {
            id: peer_id,
            last_seen_ms: now,
            pk_bound: false,
            pk_bound_at_ms: None,
            capacity: capacity.unwrap_or_default(),
            role: PeerRole::Worker,
        });
        e.last_seen_ms = now;
        if let Some(c) = capacity {
            e.capacity = c;
        }
        if has_pk && !e.pk_bound {
            e.pk_bound = true;
            e.pk_bound_at_ms = Some(now);
            let mut first = self.first_pk_bound_ms.lock();
            if first.is_none() {
                *first = Some(now);
            }
        } else if has_pk {
            e.pk_bound = true;
        }
        info!(target: "nosd", "mesh HB peer={} pk={}", peer_id, e.pk_bound);
        drop(g);
        self.run_election_stub();
    }

    /// Avança contador de settle (chamar no tick Hermes / mesh loop).
    pub fn tick(&self) {
        *self.tofu_ticks.lock() += 1;
    }

    pub fn tofu_ticks(&self) -> u32 {
        *self.tofu_ticks.lock()
    }

    pub fn peer_tofu_count(&self) -> usize {
        self.peers.lock().values().filter(|p| p.pk_bound).count()
    }

    /// ROLE / SkillSync (FRAG lab) só após janela TOFU (130 ticks ou `TOFU_SETTLE_MS`).
    pub fn may_send_control_sync(&self) -> bool {
        self.tofu_settled()
    }

    /// Eleição stub: maior `CapacityScore::total` vira Master; demais Worker.
    pub fn run_election_stub(&self) -> PeerRole {
        let mut best_id = self.local_id;
        let mut best = self.local_capacity.total();
        {
            let g = self.peers.lock();
            for p in g.values() {
                let t = p.capacity.total();
                if t > best || (t == best && p.id < best_id) {
                    best = t;
                    best_id = p.id;
                }
            }
        }
        let role = if best_id == self.local_id {
            PeerRole::Master
        } else {
            PeerRole::Worker
        };
        *self.local_role.lock() = role;
        let mut g = self.peers.lock();
        for p in g.values_mut() {
            p.role = if p.id == best_id {
                PeerRole::Master
            } else {
                PeerRole::Worker
            };
        }
        role
    }

    pub fn local_role(&self) -> PeerRole {
        *self.local_role.lock()
    }

    pub fn local_capacity(&self) -> CapacityScore {
        self.local_capacity
    }

    pub fn peer_role(&self, peer_id: u32) -> Option<PeerRole> {
        self.peers.lock().get(&peer_id).map(|p| p.role)
    }

    /// True quando há ≥1 peer PK-bound e passou a janela desde o 1º bind.
    pub fn tofu_settled(&self) -> bool {
        if self.peer_tofu_count() == 0 {
            return false;
        }
        if self.tofu_ticks() >= TOFU_SETTLE_TICKS {
            return true;
        }
        let first = *self.first_pk_bound_ms.lock();
        match first {
            Some(t0) => now_ms().saturating_sub(t0) >= TOFU_SETTLE_MS,
            None => false,
        }
    }

    pub fn peer_count(&self) -> u32 {
        self.peers.lock().len() as u32
    }

    pub fn set_local_fleet_health(&self, h: MachineHealth) {
        *self.local_fleet.lock() = Some(h);
    }

    pub fn encode_laya_intent(&self, intent: &LayaIntent) -> Vec<u8> {
        intent.encode()
    }

    /// Agenda LayaIntent para TX no próximo `MeshUdp::tick_tx` (M.2 residual).
    pub fn queue_laya_intent(&self, intent: LayaIntent) {
        *self.last_laya.lock() = Some(intent);
    }

    pub fn last_laya_intent(&self) -> Option<LayaIntent> {
        self.last_laya.lock().clone()
    }

    /// Peers → `nos_cortex::PeerCapacity` shape (M.4 offload).
    pub fn peers_for_offload(&self) -> Vec<(u32, u64, u64, bool)> {
        self.peers
            .lock()
            .values()
            .map(|p| {
                let unreachable = now_ms().saturating_sub(p.last_seen_ms) > 60_000;
                (
                    p.id,
                    u64::from(p.capacity.mem_mib),
                    u64::from(p.capacity.cpu_units),
                    unreachable,
                )
            })
            .collect()
    }

    /// Datagramas auxiliares a enviar com o HB (MCH + LayaIntent).
    pub fn encode_tick_extras(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if let Some(ref h) = *self.local_fleet.lock() {
            out.push(encode_mch(h));
        }
        if let Some(li) = self.last_laya_intent() {
            out.push(li.encode());
        }
        out
    }

    /// Máquinas para `fleet_worst` (local + peers com MCH).
    pub fn fleet_machines(&self) -> Vec<MachineHealth> {
        let mut out = Vec::new();
        if let Some(ref h) = *self.local_fleet.lock() {
            out.push(h.clone());
        }
        for h in self.peer_fleet.lock().values() {
            out.push(h.clone());
        }
        out
    }

    pub fn fleet_verdict(&self) -> Option<HealthVerdict> {
        let ms = self.fleet_machines();
        if ms.is_empty() {
            None
        } else {
            Some(fleet_worst(&ms))
        }
    }

    pub fn encode_heartbeat(&self) -> Vec<u8> {
        let mut v = MAGIC_HB.to_vec();
        v.extend_from_slice(&self.local_id.to_le_bytes());
        let seq = self
            .hb_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.persist_hb_seq(seq);
        v.extend_from_slice(&seq.to_le_bytes());
        v.extend_from_slice(b"PK\0");
        v.extend_from_slice(b"CAP\0");
        v.extend_from_slice(&self.local_capacity.cpu_units.to_le_bytes());
        v.extend_from_slice(&self.local_capacity.mem_mib.to_le_bytes());
        v.extend_from_slice(&self.local_capacity.net_score.to_le_bytes());
        if let Some(ref id) = self.ident {
            let pk = id.public_key();
            v.extend_from_slice(ident::KEY_MARK);
            v.extend_from_slice(&pk);
            let sig_off = v.len();
            v.extend_from_slice(ident::SIG_MARK);
            let sig = id.sign(&v[..sig_off]);
            v.extend_from_slice(&sig);
        }
        v
    }

    /// M6: HB assinado (`KEY\0 pk SIG\0 sig`). `None` se for HB antigo.
    fn parse_signed_hb(buf: &[u8]) -> Option<SignedHb<'_>> {
        if buf.len() < 7 + 8 + 36 + 68 || &buf[0..3] != MAGIC_HB {
            return None;
        }
        let key_pos = buf.windows(4).position(|w| w == ident::KEY_MARK)?;
        let sig_off = key_pos + 4 + 32;
        if buf.len() < sig_off + 4 + 64 || &buf[sig_off..sig_off + 4] != ident::SIG_MARK {
            return None;
        }
        if buf.len() != sig_off + 4 + 64 {
            return None;
        }
        let id = u32::from_le_bytes(buf[3..7].try_into().ok()?);
        let seq = u64::from_le_bytes(buf[7..15].try_into().ok()?);
        Some(SignedHb {
            id,
            seq,
            pk: &buf[key_pos + 4..sig_off],
            msg: &buf[..sig_off],
            sig: &buf[sig_off + 4..],
            cap: parse_hb_capacity(buf),
        })
    }

    fn on_hb_signed(&self, hb: SignedHb<'_>) {
        let Some(ref ident) = self.ident else {
            // Sem identidade local: aceitar como HB normal (lab).
            self.on_heartbeat_inner(hb.id, true, hb.cap);
            return;
        };
        if !ident::verify(hb.pk, hb.msg, hb.sig) {
            warn!(target: "nosd", "mesh HB assinatura invalida id={}", hb.id);
            return;
        }
        if !ident.allow(hb.id) {
            warn!(target: "nosd", "mesh HB id={} fora da allowlist", hb.id);
            return;
        }
        if let Err(e) = self.pinned.check(hb.id, hb.pk, hb.seq) {
            warn!(target: "nosd", "mesh HB id={} rejeitado ({e:?})", hb.id);
            return;
        }
        self.on_heartbeat_inner(hb.id, true, hb.cap);
    }

    pub fn try_parse_heartbeat(buf: &[u8]) -> Option<(u32, bool)> {
        Self::try_parse_heartbeat_full(buf).map(|(id, pk, _)| (id, pk))
    }

    /// `CAP\0` + 3×u32 LE após `PK\0` quando presente (eleição por capacity real).
    pub fn try_parse_heartbeat_full(buf: &[u8]) -> Option<(u32, bool, Option<CapacityScore>)> {
        if buf.len() < 7 || &buf[0..3] != MAGIC_HB {
            return None;
        }
        let id = u32::from_le_bytes(buf[3..7].try_into().ok()?);
        let has_pk = buf.windows(3).any(|w| w == b"PK\0");
        let cap = parse_hb_capacity(buf);
        Some((id, has_pk, cap))
    }

    /// ROLE\0{target_le32}{role_u8} — None se TOFU ainda não settle.
    pub fn try_encode_role(&self, target: u32, role: PeerRole) -> Option<Vec<u8>> {
        if !self.may_send_control_sync() {
            return None;
        }
        Some(self.encode_role_unchecked(target, role))
    }

    pub fn encode_role_unchecked(&self, target: u32, role: PeerRole) -> Vec<u8> {
        let role_u8 = match role {
            PeerRole::Master => 0,
            PeerRole::Worker => 1,
            PeerRole::Memory => 2,
            PeerRole::Compute => 3,
        };
        let mut v = MAGIC_ROLE.to_vec();
        v.extend_from_slice(&target.to_le_bytes());
        v.push(role_u8);
        v
    }

    pub fn try_parse_role(buf: &[u8]) -> Option<(u32, PeerRole)> {
        if buf.len() < 10 || &buf[0..5] != MAGIC_ROLE {
            return None;
        }
        let target = u32::from_le_bytes(buf[5..9].try_into().ok()?);
        let role = match buf[9] {
            0 => PeerRole::Master,
            2 => PeerRole::Memory,
            3 => PeerRole::Compute,
            _ => PeerRole::Worker,
        };
        Some((target, role))
    }

    pub fn encode_frack(&self, stream_id: u32, seq: u32) -> Vec<u8> {
        Frack { stream_id, seq }.encode()
    }

    pub fn take_pending_fracks(&self) -> Vec<Frack> {
        std::mem::take(&mut *self.pending_fracks.lock())
    }

    pub fn fracks_received(&self) -> Vec<Frack> {
        self.fracks_received.lock().clone()
    }

    /// Processa datagrama: HB, FRACK ou FRAG. Retorna Some(payload) se FRAG completo.
    pub fn on_datagram(&self, buf: &[u8]) -> Option<Vec<u8>> {
        if let Some(h) = parse_mch(buf) {
            let peer = u32::from(h.node_id);
            if peer != self.local_id {
                self.peer_fleet.lock().insert(peer, h);
            }
            return None;
        }
        if let Some(li) = LayaIntent::decode(buf) {
            if li.node_id != self.local_id {
                info!(
                    target: "nosd",
                    "mesh LayaIntent peer={} arm={} fnv={:016x}",
                    li.node_id,
                    li.intent_arm,
                    li.text_fnv
                );
                *self.last_laya.lock() = Some(li);
            }
            return None;
        }
        if let Some(hb) = Self::parse_signed_hb(buf) {
            if hb.id != self.local_id {
                self.on_hb_signed(hb);
            }
            return None;
        }
        if let Some((id, pk, cap)) = Self::try_parse_heartbeat_full(buf) {
            if id != self.local_id {
                if self
                    .ident
                    .as_ref()
                    .map(|i| i.require_sig())
                    .unwrap_or(false)
                {
                    warn!(
                        target: "nosd",
                        "mesh HB id={id} sem assinatura (NOSD_MESH_REQUIRE_SIG=1) - ignorado"
                    );
                } else {
                    self.on_heartbeat_inner(id, pk, cap);
                }
            }
            return None;
        }
        if let Some((target, role)) = Self::try_parse_role(buf) {
            if target == self.local_id {
                *self.local_role.lock() = role;
            }
            return None;
        }
        if let Some(ack) = Frack::parse(buf) {
            self.fracks_received.lock().push(ack);
            info!(
                target: "nosd",
                "mesh FRACK stream={} seq={}",
                ack.stream_id,
                ack.seq
            );
            return None;
        }
        if let Some((h, _payload)) = FragReassembler::parse(buf) {
            self.pending_fracks
                .lock()
                .push(Frack::from_frag(h.msg_id, h.frag_idx));
        }
        if let Some(payload) = self.frags.push(buf) {
            info!(
                target: "nosd",
                "mesh FRAG complete bytes={}",
                payload.len()
            );
            *self.last_frag.lock() = Some(payload.clone());
            return Some(payload);
        }
        None
    }

    pub fn split_frag(&self, msg_id: u32, payload: &[u8], chunk: usize) -> Vec<Vec<u8>> {
        FragReassembler::split(msg_id, payload, chunk)
    }

    pub fn last_frag_len(&self) -> usize {
        self.last_frag.lock().as_ref().map(|v| v.len()).unwrap_or(0)
    }

    /// V10 HUD: topologia pequena para o grafo da mesh (ids, papéis,
    /// capacidades e estado TOFU). Sem chaves privadas; só metadatos.
    ///
    /// Locks: `tofu_settled`/`peer_tofu_count` e `last_frag` são lidos antes
    /// do lock de `peers` (evita deadlock por lock ordering).
    pub fn topology(&self) -> serde_json::Value {
        let now = now_ms();
        let tofu_settled = self.tofu_settled();
        let tofu_peers = self.peer_tofu_count();
        let last_frag = self.last_frag_len();
        let pending_fracks = self.pending_fracks.lock().len();
        let local_role = self.local_role();
        let local = json!({
            "id": self.local_id,
            "role": serde_json::to_value(local_role).unwrap_or(json!("Worker")),
            "cpu": self.local_capacity.cpu_units,
            "mem_mib": self.local_capacity.mem_mib,
            "net": self.local_capacity.net_score,
            "tofu_settled": tofu_settled,
            "tofu_peers": tofu_peers,
            "last_frag": last_frag,
            "pending_fracks": pending_fracks,
            "local": true,
        });
        let peers: Vec<serde_json::Value> = {
            let peers = self.peers.lock();
            peers
                .values()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "role": serde_json::to_value(p.role).unwrap_or(json!("Worker")),
                        "pk_bound": p.pk_bound,
                        "last_seen_ms": p.last_seen_ms,
                        "age_ms": now.saturating_sub(p.last_seen_ms),
                        "cpu": p.capacity.cpu_units,
                        "mem_mib": p.capacity.mem_mib,
                        "net": p.capacity.net_score,
                        "local": false,
                    })
                })
                .collect()
        };
        json!({
            "local": local,
            "peers": peers,
            "verdict": self.fleet_verdict().map(|v| v.as_str()),
            "machines": self.fleet_machines().len(),
        })
    }

    pub fn health_json(&self) -> String {
        format!(
            r#"[{{"local":{},"peers":{},"tofu":{},"last_frag":{},"role":"{:?}","pending_fracks":{}}}]"#,
            self.local_id,
            self.peer_count(),
            self.tofu_settled() as u8,
            self.last_frag_len(),
            self.local_role(),
            self.pending_fracks.lock().len()
        )
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_hb_capacity(buf: &[u8]) -> Option<CapacityScore> {
    let pk_off = buf.windows(3).position(|w| w == b"PK\0")?;
    let mut off = pk_off + 3;
    if off + 4 > buf.len() || &buf[off..off + 4] != b"CAP\0" {
        return None;
    }
    off += 4;
    if off + 12 > buf.len() {
        return None;
    }
    let cpu = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    off += 4;
    let mem = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    off += 4;
    let net = u32::from_le_bytes(buf[off..off + 4].try_into().ok()?);
    Some(CapacityScore {
        cpu_units: cpu,
        mem_mib: mem,
        net_score: net,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_reports_local_identity_without_peers() {
        let m = MeshEngine::with_capacity(
            15,
            CapacityScore {
                cpu_units: 8,
                mem_mib: 4096,
                net_score: 200,
            },
        );
        let t = m.topology();
        assert_eq!(t["local"]["id"].as_u64(), Some(15));
        assert_eq!(t["local"]["role"].as_str(), Some("Worker"));
        assert_eq!(t["local"]["cpu"].as_u64(), Some(8));
        assert_eq!(t["peers"].as_array().map(|v| v.len()), Some(0));
        assert_eq!(t["machines"].as_u64(), Some(0));
    }

    #[test]
    fn hb_roundtrip_with_capacity() {
        let m = MeshEngine::with_capacity(
            15,
            CapacityScore {
                cpu_units: 8,
                mem_mib: 4096,
                net_score: 200,
            },
        );
        let buf = m.encode_heartbeat();
        let (id, pk, cap) = MeshEngine::try_parse_heartbeat_full(&buf).unwrap();
        assert_eq!(id, 15);
        assert!(pk);
        let cap = cap.unwrap();
        assert_eq!(cap.cpu_units, 8);
        assert_eq!(cap.mem_mib, 4096);
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_count(), 1);
        assert!(!m.tofu_settled());
    }

    #[test]
    fn tofu_settled_after_ticks() {
        let m = MeshEngine::new(1);
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_tofu_count(), 1);
        assert!(!m.tofu_settled());
        for _ in 0..TOFU_SETTLE_TICKS {
            m.tick();
        }
        assert!(m.tofu_settled());
    }

    #[test]
    fn role_encode_gated_until_tofu() {
        let m = MeshEngine::new(1);
        assert!(m.try_encode_role(1, PeerRole::Master).is_none());
        m.on_heartbeat(2, true);
        for _ in 0..TOFU_SETTLE_TICKS {
            m.tick();
        }
        let role = m.try_encode_role(1, PeerRole::Master).unwrap();
        assert_eq!(&role[0..5], MAGIC_ROLE);
    }

    #[test]
    fn frack_encode_wire() {
        let m = MeshEngine::new(1);
        let buf = m.encode_frack(42, 7);
        assert_eq!(buf.len(), frack::HDR_LEN);
        let parsed = Frack::parse(&buf).unwrap();
        assert_eq!(parsed.stream_id, 42);
        assert_eq!(parsed.seq, 7);
    }

    #[test]
    fn election_uses_hb_capacity() {
        let local = MeshEngine::with_capacity(
            10,
            CapacityScore {
                cpu_units: 32,
                mem_mib: 8192,
                net_score: 100,
            },
        );
        let peer_hb = MeshEngine::with_capacity(
            20,
            CapacityScore {
                cpu_units: 4,
                mem_mib: 1024,
                net_score: 50,
            },
        );
        let buf = peer_hb.encode_heartbeat();
        local.on_datagram(&buf);
        assert_eq!(local.local_role(), PeerRole::Master);
        assert_eq!(local.peer_role(20), Some(PeerRole::Worker));
    }

    #[test]
    fn frag_via_engine() {
        let m = MeshEngine::new(1);
        let parts = m.split_frag(7, b"abcdEFGH", 3);
        let mut n = 0;
        for p in parts {
            if m.on_datagram(&p).is_some() {
                n += 1;
            }
        }
        assert_eq!(n, 1);
        assert_eq!(m.last_frag_len(), 8);
    }

    #[test]
    fn frack_on_partial_frag() {
        let m = MeshEngine::new(1);
        let parts = m.split_frag(5, b"0123456789", 4);
        assert!(parts.len() > 1);
        let _ = m.on_datagram(&parts[0]);
        let pending = m.take_pending_fracks();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stream_id, 5);
        assert_eq!(pending[0].seq, 0);
        let ack = m.encode_frack(5, 0);
        m.on_datagram(&ack);
        assert_eq!(m.fracks_received().len(), 1);
    }

    #[test]
    fn election_highest_capacity_master() {
        let local = MeshEngine::with_capacity(
            10,
            CapacityScore {
                cpu_units: 2,
                mem_mib: 512,
                net_score: 10,
            },
        );
        local.on_heartbeat_with_capacity(
            20,
            true,
            CapacityScore {
                cpu_units: 16,
                mem_mib: 8192,
                net_score: 500,
            },
        );
        assert_eq!(local.local_role(), PeerRole::Worker);
        assert_eq!(local.peer_role(20), Some(PeerRole::Master));
    }

    #[test]
    fn laya_intent_datagram() {
        let m = MeshEngine::new(1);
        let li = LayaIntent {
            node_id: 2,
            clock: 1,
            intent_arm: 4,
            confidence_q8: 180,
            margin_q8: 20,
            text_fnv: content_fingerprint("mesh lab"),
        };
        m.on_datagram(&li.encode());
        let got = m.last_laya_intent().unwrap();
        assert_eq!(got.node_id, 2);
        assert_eq!(got.intent_arm, 4);
    }

    #[test]
    fn tick_extras_mch_and_laya() {
        let m = MeshEngine::new(7);
        m.set_local_fleet_health(MachineHealth {
            node_id: 7,
            sys: HealthVerdict::Go,
            audio: HealthVerdict::Unknown,
            hub: HealthVerdict::Go,
            reason: "test".into(),
        });
        m.queue_laya_intent(LayaIntent {
            node_id: 7,
            clock: 3,
            intent_arm: 1,
            confidence_q8: 200,
            margin_q8: 10,
            text_fnv: 0xabc,
        }); // types: node_id u32, clock u64
        let extras = m.encode_tick_extras();
        assert_eq!(extras.len(), 2);
        assert!(parse_mch(&extras[0]).is_some());
        assert!(LayaIntent::decode(&extras[1]).is_some());
    }

    #[test]
    fn local_wins_election() {
        let m = MeshEngine::with_capacity(
            1,
            CapacityScore {
                cpu_units: 64,
                mem_mib: 32_768,
                net_score: 1000,
            },
        );
        m.on_heartbeat_with_capacity(
            2,
            true,
            CapacityScore {
                cpu_units: 1,
                mem_mib: 256,
                net_score: 1,
            },
        );
        assert_eq!(m.local_role(), PeerRole::Master);
    }

    /// M5/ADR-0081: o settle do TOFU (janela de 7.2s) destranca ROLE/SkillSync.
    #[test]
    fn tofu_settles_after_window() {
        let m = MeshEngine::new(1);
        assert!(!m.tofu_settled(), "sem peers nao ha settle");
        m.on_heartbeat(2, true);
        assert_eq!(m.peer_tofu_count(), 1, "HB com PK -> peer pk-bound");
        assert!(!m.tofu_settled(), "recem-ligado ainda nao settle");
        std::thread::sleep(std::time::Duration::from_millis(TOFU_SETTLE_MS + 300));
        assert!(m.tofu_settled(), "deve settle ao fim de TOFU_SETTLE_MS");
    }

    #[test]
    fn crdt_log_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        {
            let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
            m.crdt_put(1, "k", "v", 0);
            m.crdt_put(1, "k2", "v2", 1);
        }
        let m2 = MeshEngine::new(1).with_crdt_log(&path).unwrap();
        assert_eq!(m2.crdt_get("k").map(|e| e.value), Some("v".to_string()));
        assert_eq!(m2.crdt_get("k2").map(|e| e.value), Some("v2".to_string()));
        assert_eq!(m2.crdt_len(), 2);
    }

    #[test]
    fn crdt_log_compacts_and_tolerates_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        {
            let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
            for i in 0..100u64 {
                m.crdt_put(1, "k", &format!("v{i}"), i);
            }
        }
        // Simula crash a meio de um write.
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "{{\"key\": \"trunc").unwrap();
        }
        let before = std::fs::read_to_string(&path).unwrap().lines().count();
        assert!(
            before > CRDT_COMPACT_LINES,
            "log devia estar grande: {before}"
        );

        let m2 = MeshEngine::new(1).with_crdt_log(&path).unwrap();
        assert_eq!(m2.crdt_len(), 1, "uma chave (LWW)");
        assert_eq!(m2.crdt_get("k").map(|e| e.value), Some("v99".to_string()));
        let after = std::fs::read_to_string(&path).unwrap().lines().count();
        assert!(after <= 2, "log devia ficar compacto: {after}");
    }

    #[test]
    fn crdt_log_compacts_at_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
        for i in 0..100u64 {
            m.crdt_put(1, "k", &format!("v{i}"), i);
        }
        assert!(m.crdt_maybe_compact().unwrap(), "devia compactar");
        let lines = std::fs::read_to_string(&path).unwrap().lines().count();
        assert!(lines <= 2, "log apos compactacao: {lines}");
        // Escrever depois da compactacao continua a funcionar.
        m.crdt_put(1, "k2", "x", 200);
        assert!(!m.crdt_maybe_compact().unwrap(), "ja compactado");
        let m2 = MeshEngine::new(1).with_crdt_log(&path).unwrap();
        assert_eq!(m2.crdt_len(), 2, "k e k2");
        assert_eq!(m2.crdt_get("k").map(|e| e.value), Some("v99".to_string()));
        assert_eq!(m2.crdt_get("k2").map(|e| e.value), Some("x".to_string()));
    }

    #[test]
    fn crdt_log_rejects_tampered_lines() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        let id = Identity::from_seed([77u8; 32], HashSet::new(), true);
        {
            let m = MeshEngine::with_identity(1, CapacityScore::default(), id.clone())
                .with_crdt_log(&path)
                .unwrap();
            m.crdt_put(1, "k", "genuino", 0);
        }
        // Adultera: append de uma linha com a mesma chave e assinatura falsa.
        {
            use base64::Engine as _;
            use std::io::Write;
            let forged = serde_json::json!({
                "key": "k", "value": "malicioso", "vv": {"1": 99}, "writer": 1, "tick": 99
            });
            let fake = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "{}\t{}", forged, fake).unwrap();
        }
        let m2 = MeshEngine::with_identity(1, CapacityScore::default(), id)
            .with_crdt_log(&path)
            .unwrap();
        assert_eq!(
            m2.crdt_get("k").map(|e| e.value),
            Some("genuino".to_string()),
            "linha adulterada nao pode substituir a genuina"
        );
    }

    #[test]
    fn crdt_log_detects_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        {
            let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
            // > CRDT_ANCHOR_EVERY: o checkpoint periodico corre durante o uso,
            // sem depender de um reopen que actualize a ancora.
            for i in 0..40u64 {
                m.crdt_put(1, &format!("k{i}"), "v", i);
            }
        }
        let anchor = std::fs::read_to_string(path.with_extension("anchor")).unwrap();
        assert!(anchor.split_whitespace().count() == 2, "ancora: {anchor}");
        let full = std::fs::read(&path).unwrap();
        assert!(full.len() > 20);
        // Rollback: repor apenas metade das linhas (estado antigo).
        let half = full
            .split_inclusive(|b| *b == b'\n')
            .take(20)
            .flatten()
            .copied()
            .collect::<Vec<u8>>();
        std::fs::write(&path, &half).unwrap();
        let r = MeshEngine::new(1).with_crdt_log(&path);
        assert!(r.is_err(), "rollback devia ser detetado");
        assert!(format!("{:?}", r.err().unwrap()).contains("rollback"));
    }

    /// T1.1: com identidade, a ancora e assinada; versões forjadas (sem
    /// assinatura) são recusadas — quem escreve no data-dir deixa de
    /// conseguir fabricar um rollback com ancora nova. Sem identidade
    /// (lab), o formato antigo de 2 campos mantém-se.
    #[test]
    fn crdt_log_anchor_signed_and_forgery_refused() {
        use std::collections::HashSet;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        let anchor_path = path.with_extension("anchor");
        {
            let m = MeshEngine::with_identity(
                1,
                CapacityScore::default(),
                Identity::from_seed([77u8; 32], HashSet::new(), true),
            )
            .with_crdt_log(&path)
            .unwrap();
            for i in 0..40u64 {
                m.crdt_put(1, &format!("k{i}"), "v", i);
            }
        }
        let valid = std::fs::read_to_string(&anchor_path).unwrap();
        assert_eq!(
            valid.split_whitespace().count(),
            3,
            "ancora assinada tem 3 campos: {valid}"
        );
        // Ancora valida reabre.
        assert!(MeshEngine::with_identity(
            1,
            CapacityScore::default(),
            Identity::from_seed([77u8; 32], HashSet::new(), true),
        )
        .with_crdt_log(&path)
        .is_ok());
        // Forjar ancora sem assinatura: recusada (fail-closed).
        std::fs::write(&anchor_path, "999999 999999").unwrap();
        let r = MeshEngine::with_identity(
            1,
            CapacityScore::default(),
            Identity::from_seed([77u8; 32], HashSet::new(), true),
        )
        .with_crdt_log(&path);
        assert!(r.is_err(), "ancora sem assinatura devia ser recusada");
        assert!(format!("{:?}", r.err().unwrap()).contains("assinatura"));
        // Sem identidade: formato antigo (2 campos), compat lab.
        let dir2 = tempfile::tempdir().unwrap();
        let path2 = dir2.path().join("crdt.jsonl");
        {
            let m = MeshEngine::new(1).with_crdt_log(&path2).unwrap();
            m.crdt_put(1, "k", "v", 1);
        }
        let a2 = std::fs::read_to_string(path2.with_extension("anchor")).unwrap();
        assert_eq!(a2.split_whitespace().count(), 2, "{a2}");
    }

    /// T1.3: o seq do HB arranca de max(epoch_ms, persistido+1); a janela de
    /// "Replay para sempre" com relógio a recuar entre restarts fecha.
    #[test]
    fn hb_seq_store_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mesh").join("hb.seq");
        let e1 = MeshEngine::new(1).with_hb_seq_store(&path).unwrap();
        for _ in 0..20 {
            let _ = e1.encode_heartbeat();
        }
        let stored: u64 = std::fs::read_to_string(&path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(stored > 0, "seq persistido num checkpoint de 16");
        let e2 = MeshEngine::new(1).with_hb_seq_store(&path).unwrap();
        assert!(
            e2.hb_seq() > stored,
            "restart arranca a seguir ao seq persistido ({} -> {})",
            stored,
            e2.hb_seq()
        );
    }

    #[test]
    fn crdt_log_accepts_growth_and_compaction() {
        // Nao ha falso positivo: crescer depois da ancora e compactar (que
        // anuncia a ancora nova antes do rename) continuam a abrir bem.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.jsonl");
        {
            let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
            for i in 0..70u64 {
                m.crdt_put(1, "k", &format!("v{i}"), i);
            }
        }
        let m = MeshEngine::new(1).with_crdt_log(&path).unwrap();
        assert_eq!(m.crdt_len(), 1);
        assert_eq!(m.crdt_get("k").map(|e| e.value), Some("v69".into()));
    }

    #[test]
    fn crdt_log_nsgdb_roundtrip_not_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nsgdb").join("crdt").join("seg.bin");
        {
            let m = MeshEngine::new(1)
                .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
                .unwrap();
            m.crdt_put(1, "k", "v", 0);
            m.crdt_put(1, "k2", "v2", 1);
        }
        let raw = std::fs::read(&path).unwrap();
        assert!(raw.starts_with(&CRDT_SEG_MAGIC), "magic NSGC");
        assert!(!raw.windows(2).any(|w| w == b"\n{"), "segmento nao e JSONL");
        let m2 = MeshEngine::new(1)
            .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
            .unwrap();
        assert_eq!(m2.crdt_len(), 2);
        assert_eq!(m2.crdt_get("k2").map(|e| e.value), Some("v2".into()));
    }

    #[test]
    fn crdt_log_nsgdb_detects_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nsgdb").join("crdt").join("seg.bin");
        {
            let m = MeshEngine::new(1)
                .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
                .unwrap();
            for i in 0..40u64 {
                m.crdt_put(1, &format!("k{i}"), "v", i);
            }
        }
        let anchor = std::fs::read_to_string(path.with_extension("anchor")).unwrap();
        assert_eq!(anchor.split_whitespace().count(), 2, "ancora: {anchor}");
        let raw = std::fs::read(&path).unwrap();
        // Repor so o cabecalho: 0 registos < 32 da ancora -> rollback.
        std::fs::write(&path, &raw[..16]).unwrap();
        let r = MeshEngine::new(1).with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb);
        assert!(r.is_err(), "rollback devia ser detetado");
        assert!(format!("{:?}", r.err().unwrap()).contains("rollback"));
    }

    #[test]
    fn crdt_log_nsgdb_rejects_tampered_record() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nsgdb").join("crdt").join("seg.bin");
        let id = Identity::from_seed([78u8; 32], HashSet::new(), true);
        {
            let m = MeshEngine::with_identity(1, CapacityScore::default(), id.clone())
                .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
                .unwrap();
            m.crdt_put(1, "k", "genuino", 0);
        }
        {
            use base64::Engine as _;
            use std::io::Write;
            let forged = serde_json::json!({
                "key": "k", "value": "malicioso", "vv": {"1": 99}, "writer": 1, "tick": 99
            });
            let mut rec = serde_json::to_vec(&forged).unwrap();
            rec.push(b'\t');
            rec.extend_from_slice(
                base64::engine::general_purpose::STANDARD
                    .encode([0u8; 64])
                    .as_bytes(),
            );
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            f.write_all(&(rec.len() as u32).to_le_bytes()).unwrap();
            f.write_all(&rec).unwrap();
        }
        let m2 = MeshEngine::with_identity(1, CapacityScore::default(), id)
            .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
            .unwrap();
        assert_eq!(
            m2.crdt_get("k").map(|e| e.value),
            Some("genuino".to_string()),
            "registo adulterado nao pode substituir o genuino"
        );
    }

    #[test]
    fn crdt_log_nsgdb_compacts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nsgdb").join("crdt").join("seg.bin");
        {
            let m = MeshEngine::new(1)
                .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
                .unwrap();
            for i in 0..70u64 {
                m.crdt_put(1, "k", &format!("v{i}"), i);
            }
        }
        let before = std::fs::read(&path).unwrap();
        let m = MeshEngine::new(1)
            .with_crdt_log_fmt(&path, CrdtLogFormat::Nsgdb)
            .unwrap();
        assert_eq!(m.crdt_len(), 1);
        assert_eq!(m.crdt_get("k").map(|e| e.value), Some("v69".into()));
        let (_, recs) = read_log(&path, CrdtLogFormat::Nsgdb).unwrap();
        assert_eq!(recs.len(), 1, "compactado a 1 registo");
        let after = std::fs::read(&path).unwrap();
        assert!(after.len() < before.len(), "compactacao devia encolher");
        assert!(after.starts_with(&CRDT_SEG_MAGIC));
    }

    #[test]
    fn hb_seq_is_wall_clock_based_across_restart() {
        // M5.1: o seq do HB tem de ser grande (epoch ms) para um processo
        // reiniciado nao ser tratado como replay pelos peers.
        let a = MeshEngine::new(1);
        let h1 = a.encode_heartbeat();
        let seq1 = u64::from_le_bytes(h1[7..15].try_into().unwrap());
        assert!(seq1 > 1_600_000_000_000, "seq base: {seq1}");
        let h2 = a.encode_heartbeat();
        let seq2 = u64::from_le_bytes(h2[7..15].try_into().unwrap());
        assert!(seq2 > seq1, "dentro do processo tem de ser monotono");
        // "Restart" (>=1 ms depois): novo engine -> seq maior que o anterior.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = MeshEngine::new(1);
        let seq3 = u64::from_le_bytes(b.encode_heartbeat()[7..15].try_into().unwrap());
        assert!(seq3 > seq2, "restart: {seq3} <= {seq2}");
    }

    #[test]
    fn signed_hb_is_verified_and_pinned() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let a = Identity::from_seed([11u8; 32], HashSet::new(), true);
        let b = Identity::from_seed([12u8; 32], HashSet::new(), true);
        let ma = MeshEngine::with_identity(11, CapacityScore::default(), a);
        let mb = MeshEngine::with_identity(12, CapacityScore::default(), b);
        let hb = ma.encode_heartbeat();
        mb.on_datagram(&hb);
        assert_eq!(mb.peer_count(), 1, "HB assinado deve registar peer");
        assert_eq!(mb.pinned_peers(), 1, "deve fixar (TOFU) a chave");
    }

    #[test]
    fn tampered_hb_is_rejected() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let a = Identity::from_seed([21u8; 32], HashSet::new(), true);
        let b = Identity::from_seed([22u8; 32], HashSet::new(), true);
        let ma = MeshEngine::with_identity(21, CapacityScore::default(), a);
        let mb = MeshEngine::with_identity(22, CapacityScore::default(), b);
        let mut hb = ma.encode_heartbeat();
        let mid = hb.len() / 2;
        hb[mid] ^= 0x01;
        mb.on_datagram(&hb);
        assert_eq!(mb.peer_count(), 0, "HB adulterado nao pode entrar");
        assert_eq!(mb.pinned_peers(), 0);
    }

    #[test]
    fn unsigned_hb_rejected_when_signature_required() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let plain = MeshEngine::with_capacity(31, CapacityScore::default());
        let a = Identity::from_seed([32u8; 32], HashSet::new(), true);
        let guarded = MeshEngine::with_identity(31, CapacityScore::default(), a);
        let hb = plain.encode_heartbeat();
        guarded.on_datagram(&hb);
        assert_eq!(
            guarded.peer_count(),
            0,
            "REQUIRE_SIG=1 ignora HB sem assinatura"
        );
    }

    #[test]
    fn allowlist_blocks_unknown_signed_peer() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let a = Identity::from_seed([41u8; 32], HashSet::new(), true);
        let b = Identity::from_seed([42u8; 32], HashSet::new(), true);
        let b_id = b.node_id();
        let ma = MeshEngine::with_identity(41, CapacityScore::default(), a);
        // allowlist so com um id que nao e o do peer
        let allow: HashSet<u32> = [b_id.wrapping_add(1)].into_iter().collect();
        let mb = MeshEngine::with_identity(
            42,
            CapacityScore::default(),
            Identity::from_seed([42u8; 32], allow, true),
        );
        mb.on_datagram(&ma.encode_heartbeat());
        assert_eq!(mb.peer_count(), 0, "id fora da allowlist nao entra");
    }

    #[test]
    fn dbg_signed_wire_shape() {
        use crate::ident::Identity;
        use std::collections::HashSet;
        let a = Identity::from_seed([51u8; 32], HashSet::new(), true);
        let m = MeshEngine::with_identity(51, CapacityScore::default(), a);
        let hb = m.encode_heartbeat();
        let parsed = MeshEngine::parse_signed_hb(&hb);
        assert!(
            parsed.is_some(),
            "len={} head={:?}",
            hb.len(),
            String::from_utf8_lossy(&hb[..8.min(hb.len())])
        );
    }
}
