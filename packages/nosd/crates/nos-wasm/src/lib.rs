//! wasmi Frugal v2 — contrato de sandbox explícito (ADR-0118 / ADR-0059).
//!
//! O que mudou em relação ao "frugal" original:
//!
//! - **Política tipada** (`SandboxPolicy`): heap, fuel, páginas, tamanho de
//!   módulo e saída máxima deixam de ser constantes soltas e passam a ser um
//!   orçamento verificável.
//! - **Inspeção antes de executar** (`Inspection`): imports fora da allowlist
//!   são recusados com nome (`module::name`), não com um erro opaco do linker.
//! - **ABI v2**: além de `run(i32) -> i32`, uma app pode exportar
//!   `result_ptr() -> i32` + `result_len() -> i32` para devolver bytes do seu
//!   linear memory. O host lê no máximo `max_output_bytes` e nunca confia nos
//!   limites declarados pelo convidado.
//! - **Logs**: import `aios_log.write(level, ptr, len) -> i32`, limitado pelo
//!   mesmo orçamento de saída. Sem import de fs/net/exec.
//! - **Cache de módulos**: o `Engine` é reutilizado e módulos são indexados
//!   por BLAKE3. `FrugalRuntime::shared()` dá uma instância por processo para
//!   que `skill.run` não recompile o mesmo módulo a cada chamada.
//! - **Fuel medido**: `ExecReport.fuel_used` é o que o convidado realmente
//!   consumiu (limite `policy.fuel`), não uma estimativa.
//!
//! Honestidade: `aios_ai.infer` continua um stub de política (0 = permitido,
//! -1 = negado). Não há chamada a modelo a partir do wasm.

use nos_core::{CapBits, CapGate};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use thiserror::Error;
use wasmi::{Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// 256 KiB = 4 × 64 KiB WASM pages.
pub const FRUGAL_HEAP_BYTES: usize = 256 * 1024;
pub const FRUGAL_FUEL: u64 = 100_000;
pub const FRUGAL_MAX_PAGES: u32 = 4;
pub const FRUGAL_MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
pub const FRUGAL_MAX_OUTPUT_BYTES: usize = 16 * 1024;

/// ABI v1: só `run(i32) -> i32`.
pub const ABI_V1: u32 = 1;
/// ABI v2: `run` + `result_ptr`/`result_len` (bytes de resultado).
pub const ABI_V2: u32 = 2;

/// Imports que o host expõe. Qualquer outro par é recusado na inspeção.
pub const ALLOWED_IMPORTS: &[(&str, &str)] = &[("aios_ai", "infer"), ("aios_log", "write")];

/// Orçamento de execução. `app` é opt-in; skills usam `frugal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SandboxPolicy {
    pub heap_bytes: usize,
    pub fuel: u64,
    pub max_pages: u32,
    pub max_table_elements: u32,
    pub max_module_bytes: usize,
    pub max_output_bytes: usize,
}

impl SandboxPolicy {
    /// Perfil de skill: 256 KiB, 100k fuel, 4 páginas, 16 KiB de saída.
    pub const fn frugal() -> Self {
        Self {
            heap_bytes: FRUGAL_HEAP_BYTES,
            fuel: FRUGAL_FUEL,
            max_pages: FRUGAL_MAX_PAGES,
            max_table_elements: 1024,
            max_module_bytes: FRUGAL_MAX_MODULE_BYTES,
            max_output_bytes: FRUGAL_MAX_OUTPUT_BYTES,
        }
    }

    /// Perfil de app Wasm (opt-in): 1 MiB, 500k fuel, 16 páginas, 64 KiB.
    /// Continua sem imports de fs/net/exec — "app" não é "sem sandbox".
    pub const fn app() -> Self {
        Self {
            heap_bytes: 1024 * 1024,
            fuel: 500_000,
            max_pages: 16,
            max_table_elements: 4096,
            max_module_bytes: 8 * 1024 * 1024,
            max_output_bytes: 64 * 1024,
        }
    }
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::frugal()
    }
}

/// Import declarado por um módulo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportRef {
    pub module: String,
    pub name: String,
}

impl std::fmt::Display for ImportRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.module, self.name)
    }
}

/// Factos estáticos de um módulo (compilado, não instanciado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inspection {
    pub bytes: usize,
    pub abi: u32,
    pub imports: Vec<ImportRef>,
    pub exports: Vec<String>,
    pub denied_imports: Vec<ImportRef>,
    pub has_run: bool,
    pub has_result_buffer: bool,
    pub has_memory: bool,
}

impl Inspection {
    /// Contrato mínimo: `run` presente e nenhum import fora da allowlist.
    pub fn compatible(&self) -> bool {
        self.has_run && self.denied_imports.is_empty()
    }
}

/// Capacidades efetivamente concedidas a esta execução. `log` é separado de
/// `CapBits` porque é um canal de dados do host, não um bit de política.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grants {
    pub caps: CapBits,
    pub log: bool,
}

impl Default for Grants {
    fn default() -> Self {
        Self::NONE
    }
}

impl Grants {
    pub const NONE: Self = Self {
        caps: CapBits::NONE,
        log: false,
    };

    pub const fn new(caps: CapBits, log: bool) -> Self {
        Self { caps, log }
    }

    pub const fn caps(caps: CapBits) -> Self {
        Self { caps, log: false }
    }
}

/// Resultado observável de uma execução.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExecReport {
    pub value: i32,
    pub fuel_used: u64,
    pub fuel_limit: u64,
    pub abi: u32,
    pub output: Vec<u8>,
    pub output_truncated: bool,
    pub result: Option<Vec<u8>>,
    pub result_truncated: bool,
}

impl ExecReport {
    /// Saída de log como texto (lossy: um convidado pode escrever bytes).
    pub fn output_text(&self) -> String {
        String::from_utf8_lossy(&self.output).to_string()
    }

    /// Resultado da app como texto, se for UTF-8 válido.
    pub fn result_text(&self) -> Option<String> {
        self.result
            .as_ref()
            .and_then(|b| String::from_utf8(b.clone()).ok())
    }
}

#[derive(Debug, Error)]
pub enum WasmError {
    #[error("cap: {0}")]
    Cap(&'static str),
    #[error("module too large: {0} bytes > {1}")]
    ModuleTooLarge(usize, usize),
    #[error("wasmi: {0}")]
    Wasmi(String),
    #[error("module: {0}")]
    Module(String),
    #[error("import denied: {0}")]
    ImportDenied(String),
    #[error("contract: {0}")]
    Contract(&'static str),
    #[error("fuel exhausted")]
    Fuel,
    #[error("trap: {0}")]
    Trap(String),
}

/// Estado do host partilhado pelo armazenamento do wasmi.
pub struct HostState {
    pub gate: CapGate,
    pub current_gen: u64,
    pub allow_log: bool,
    pub out: Vec<u8>,
    pub out_truncated: bool,
    pub max_output_bytes: usize,
    pub limits: StoreLimits,
}

impl HostState {
    pub fn with_policy(grants: Grants, gen: u64, policy: &SandboxPolicy) -> Self {
        let limits = StoreLimitsBuilder::new()
            .memory_size(policy.heap_bytes)
            .memories(1)
            .tables(1)
            .table_elements(policy.max_table_elements)
            .instances(1)
            .build();
        Self {
            gate: CapGate::new(grants.caps, gen),
            current_gen: gen,
            allow_log: grants.log,
            out: Vec::new(),
            out_truncated: false,
            max_output_bytes: policy.max_output_bytes,
            limits,
        }
    }

    /// Compat: perfil frugal, sem log.
    pub fn frugal(caps: CapBits, gen: u64) -> Self {
        Self::with_policy(Grants::caps(caps), gen, &SandboxPolicy::frugal())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

struct ModuleCache {
    cap: usize,
    map: HashMap<String, Module>,
    order: VecDeque<String>,
    hits: u64,
    misses: u64,
}

impl ModuleCache {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            map: HashMap::new(),
            order: VecDeque::new(),
            hits: 0,
            misses: 0,
        }
    }

    fn get_or_compile(
        &mut self,
        engine: &Engine,
        key: &str,
        wasm: &[u8],
    ) -> Result<Module, WasmError> {
        if let Some(m) = self.map.get(key) {
            self.hits += 1;
            return Ok(m.clone());
        }
        self.misses += 1;
        let module = Module::new(engine, wasm).map_err(|e| WasmError::Module(e.to_string()))?;
        if self.map.len() >= self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        self.map.insert(key.to_string(), module.clone());
        self.order.push_back(key.to_string());
        Ok(module)
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            entries: self.map.len(),
        }
    }
}

pub struct FrugalRuntime {
    engine: Engine,
    policy: SandboxPolicy,
    cache: Mutex<ModuleCache>,
}

impl Default for FrugalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl FrugalRuntime {
    /// Perfil frugal (skills).
    pub fn new() -> Self {
        Self::with_policy(SandboxPolicy::frugal())
    }

    /// Instância partilhada por processo: o cache de módulos sobrevive a
    /// chamadas `skill.run` e o `Engine` é reutilizado.
    pub fn shared() -> &'static FrugalRuntime {
        static SHARED: OnceLock<FrugalRuntime> = OnceLock::new();
        SHARED.get_or_init(FrugalRuntime::new)
    }

    /// Perfil de app (opt-in do operador) numa segunda instância partilhada,
    /// para não contaminar o orçamento frugal das skills.
    pub fn shared_app() -> &'static FrugalRuntime {
        static APP: OnceLock<FrugalRuntime> = OnceLock::new();
        APP.get_or_init(|| FrugalRuntime::with_policy(SandboxPolicy::app()))
    }

    pub fn with_policy(policy: SandboxPolicy) -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        Self {
            engine: Engine::new(&config),
            policy,
            cache: Mutex::new(ModuleCache::new(16)),
        }
    }

    pub fn policy(&self) -> SandboxPolicy {
        self.policy
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.lock().unwrap_or_else(|p| p.into_inner()).stats()
    }

    /// Inspeção estática. Compila (e portanto valida) o módulo; não executa
    /// `start` nem expõe imports.
    pub fn inspect(&self, wasm: &[u8]) -> Result<Inspection, WasmError> {
        self.check_size(wasm.len())?;
        let module = self.compile(wasm)?;
        Ok(Self::describe(&module, wasm.len()))
    }

    fn check_size(&self, len: usize) -> Result<(), WasmError> {
        if len == 0 {
            return Err(WasmError::Module("empty module".into()));
        }
        if len > self.policy.max_module_bytes {
            return Err(WasmError::ModuleTooLarge(len, self.policy.max_module_bytes));
        }
        Ok(())
    }

    fn compile(&self, wasm: &[u8]) -> Result<Module, WasmError> {
        let key = blake3::hash(wasm).to_hex().to_string();
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        cache.get_or_compile(&self.engine, &key, wasm)
    }

    fn describe(module: &Module, bytes: usize) -> Inspection {
        let imports: Vec<ImportRef> = module
            .imports()
            .map(|i| ImportRef {
                module: i.module().to_string(),
                name: i.name().to_string(),
            })
            .collect();
        let exports: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();
        let denied_imports: Vec<ImportRef> = imports
            .iter()
            .filter(|i| !is_allowed_import(&i.module, &i.name))
            .cloned()
            .collect();
        let has_run = exports.iter().any(|e| e == "run");
        let has_result_buffer =
            exports.iter().any(|e| e == "result_ptr") && exports.iter().any(|e| e == "result_len");
        let has_memory = exports.iter().any(|e| e == "memory");
        Inspection {
            bytes,
            abi: if has_result_buffer { ABI_V2 } else { ABI_V1 },
            imports,
            exports,
            denied_imports,
            has_run,
            has_result_buffer,
            has_memory,
        }
    }

    fn install_host_imports(linker: &mut Linker<HostState>) -> Result<(), WasmError> {
        linker
            .func_wrap(
                "aios_ai",
                "infer",
                |caller: Caller<'_, HostState>| -> Result<i32, wasmi::Error> {
                    if caller
                        .data()
                        .gate
                        .allow(CapBits::INFER, caller.data().current_gen)
                        .is_err()
                    {
                        return Ok(-1);
                    }
                    // Stub de política: 0 = permitido. Nenhum modelo é chamado
                    // a partir do wasm (honestidade ADR-0113).
                    Ok(0)
                },
            )
            .map_err(|e| WasmError::Wasmi(e.to_string()))?;
        linker
            .func_wrap(
                "aios_log",
                "write",
                |mut caller: Caller<'_, HostState>,
                 level: i32,
                 ptr: i32,
                 len: i32|
                 -> Result<i32, wasmi::Error> {
                    if !caller.data().allow_log {
                        return Ok(-1);
                    }
                    if !(0..=4).contains(&level) {
                        return Ok(-1);
                    }
                    let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return Ok(-1);
                    };
                    let len = usize::try_from(len).unwrap_or(0);
                    let start = match usize::try_from(ptr) {
                        Ok(v) => v,
                        Err(_) => return Ok(-1),
                    };
                    let data = mem.data(&caller);
                    let Some(end) = start.checked_add(len) else {
                        return Ok(-1);
                    };
                    if end > data.len() {
                        return Ok(-1);
                    }
                    let remaining = caller
                        .data()
                        .max_output_bytes
                        .saturating_sub(caller.data().out.len());
                    let take = len.min(remaining);
                    let chunk = data[start..start + take].to_vec();
                    if take < len {
                        caller.data_mut().out_truncated = true;
                    }
                    caller.data_mut().out.extend_from_slice(&chunk);
                    Ok(take as i32)
                },
            )
            .map_err(|e| WasmError::Wasmi(e.to_string()))?;
        Ok(())
    }

    /// Executa `run(arg)` com a política e as capacidades dadas.
    pub fn execute(
        &self,
        wasm: &[u8],
        grants: Grants,
        gen: u64,
        arg: i32,
    ) -> Result<ExecReport, WasmError> {
        self.check_size(wasm.len())?;
        let module = self.compile(wasm)?;
        let inspection = Self::describe(&module, wasm.len());
        if let Some(denied) = inspection.denied_imports.first() {
            return Err(WasmError::ImportDenied(denied.to_string()));
        }
        if !inspection.has_run {
            return Err(WasmError::Contract("missing export run(i32) -> i32"));
        }

        let mut store = Store::new(
            &self.engine,
            HostState::with_policy(grants, gen, &self.policy),
        );
        store.limiter(|s| &mut s.limits);
        store
            .set_fuel(self.policy.fuel)
            .map_err(|e| WasmError::Wasmi(e.to_string()))?;

        let mut linker = Linker::<HostState>::new(&self.engine);
        Self::install_host_imports(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| WasmError::Trap(e.to_string()))?
            .start(&mut store)
            .map_err(classify_trap)?;

        let run = instance
            .get_typed_func::<i32, i32>(&store, "run")
            .map_err(|e| WasmError::Module(format!("run must be (i32) -> i32: {e}")))?;
        let value = run.call(&mut store, arg).map_err(classify_trap)?;

        let mut report = ExecReport {
            value,
            fuel_limit: self.policy.fuel,
            abi: inspection.abi,
            ..Default::default()
        };

        // ABI v2: o convidado publica bytes no seu linear memory. O host
        // bounds-checa e corta em `max_output_bytes`; nunca confia no len.
        if inspection.has_result_buffer {
            if let Some(mem) = instance.get_memory(&store, "memory") {
                let ptr_fn = instance
                    .get_typed_func::<(), i32>(&store, "result_ptr")
                    .ok();
                let len_fn = instance
                    .get_typed_func::<(), i32>(&store, "result_len")
                    .ok();
                if let (Some(pf), Some(lf)) = (ptr_fn, len_fn) {
                    let ptr = pf.call(&mut store, ()).map_err(classify_trap)?;
                    let len = lf.call(&mut store, ()).map_err(classify_trap)?;
                    let len = usize::try_from(len).unwrap_or(0);
                    let take = len.min(self.policy.max_output_bytes);
                    let start = usize::try_from(ptr).unwrap_or(usize::MAX);
                    let data = mem.data(&store);
                    let end = start.saturating_add(take);
                    if end > data.len() {
                        return Err(WasmError::Contract("result buffer out of bounds"));
                    }
                    report.result = Some(data[start..end].to_vec());
                    report.result_truncated = take < len;
                }
            }
        }

        let state = store.data();
        report.output = state.out.clone();
        report.output_truncated = state.out_truncated;
        let remaining = store.get_fuel().unwrap_or(0);
        report.fuel_used = self.policy.fuel.saturating_sub(remaining);
        Ok(report)
    }

    /// Compat v1: só o valor de retorno.
    pub fn run_i32(
        &self,
        wasm: &[u8],
        caps: CapBits,
        gen: u64,
        arg: i32,
    ) -> Result<i32, WasmError> {
        self.execute(wasm, Grants::caps(caps), gen, arg)
            .map(|r| r.value)
    }
}

fn is_allowed_import(module: &str, name: &str) -> bool {
    ALLOWED_IMPORTS
        .iter()
        .any(|(m, n)| *m == module && *n == name)
}

fn classify_trap(e: wasmi::Error) -> WasmError {
    let msg = e.to_string();
    if msg.to_ascii_lowercase().contains("fuel") {
        WasmError::Fuel
    } else {
        WasmError::Trap(msg)
    }
}

/// WASM: (module (func (export "run") (param i32) (result i32) (local.get 0)))
pub fn infer_probe_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (import "aios_ai" "infer" (func $infer (result i32)))
            (func (export "run") (param i32) (result i32)
                (call $infer))
        )"#,
    )
    .expect("valid wat")
}

pub fn tiny_identity_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "run") (param i32) (result i32)
                local.get 0))
        "#,
    )
    .expect("valid wat")
}

/// App ABI v2 de demonstração: devolve `arg` e publica um JSON fixo.
pub fn echo_app_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (memory (export "memory") 1)
            (data (i32.const 64) "{\"app\":\"echo\"}")
            (func (export "run") (param i32) (result i32)
                local.get 0)
            (func (export "result_ptr") (result i32) (i32.const 64))
            (func (export "result_len") (result i32) (i32.const 14)))
        "#,
    )
    .expect("valid wat")
}

/// Loop que consome fuel: `arg` iterações de add (trap se fuel estourar).
pub fn fuel_burn_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (func (export "run") (param $n i32) (result i32)
                (local $i i32)
                (local $acc i32)
                (local.set $i (i32.const 0))
                (local.set $acc (i32.const 0))
                (block $done
                    (loop $body
                        (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
                        (local.set $acc (i32.add (local.get $acc) (i32.const 1)))
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br $body)))
                (local.get $acc)))
        "#,
    )
    .expect("valid wat")
}

/// Resultado de frota frugal (medição host — ADR-0113).
#[derive(Debug, Clone)]
pub struct FleetReport {
    pub n: usize,
    pub ok: usize,
    pub fuel_traps: usize,
    pub other_err: usize,
    pub elapsed_ms: u128,
    pub heap_budget_kib: usize,
}

/// Corre `n` instâncias sequenciais do identity WASM (1 store por agente).
pub fn run_fleet_identity(n: usize) -> FleetReport {
    let rt = FrugalRuntime::new();
    let code = tiny_identity_wasm();
    let t0 = std::time::Instant::now();
    let mut ok = 0usize;
    let mut fuel_traps = 0usize;
    let mut other_err = 0usize;
    for i in 0..n {
        match rt.run_i32(&code, CapBits::FULL, 1, i as i32) {
            Ok(v) if v == i as i32 => ok += 1,
            Ok(_) => other_err += 1,
            Err(WasmError::Fuel) => fuel_traps += 1,
            Err(_) => other_err += 1,
        }
    }
    FleetReport {
        n,
        ok,
        fuel_traps,
        other_err,
        elapsed_ms: t0.elapsed().as_millis(),
        heap_budget_kib: (FRUGAL_HEAP_BYTES / 1024) * n,
    }
}

/// Provoca trap de fuel com loop grande.
pub fn expect_fuel_trap() -> bool {
    let rt = FrugalRuntime::new();
    matches!(
        rt.run_i32(&fuel_burn_wasm(), CapBits::FULL, 1, 5_000_000),
        Err(WasmError::Fuel)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aios_infer_denied_without_cap() {
        let rt = FrugalRuntime::new();
        let caps = CapBits::MEMORY.union(CapBits::FS);
        let v = rt
            .run_i32(&infer_probe_wasm(), caps, 1, 0)
            .expect("infer import");
        assert_eq!(v, -1);
    }

    #[test]
    fn aios_infer_allowed_with_infer_cap() {
        let rt = FrugalRuntime::new();
        let v = rt
            .run_i32(&infer_probe_wasm(), CapBits::INFER, 1, 0)
            .expect("infer import");
        assert_eq!(v, 0);
    }

    #[test]
    fn frugal_identity() {
        let rt = FrugalRuntime::new();
        let v = rt
            .run_i32(&tiny_identity_wasm(), CapBits::FULL, 1, 42)
            .unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn constants() {
        assert_eq!(FRUGAL_HEAP_BYTES, 256 * 1024);
        assert_eq!(FRUGAL_FUEL, 100_000);
        assert_eq!(FRUGAL_MAX_PAGES, 4);
        assert_eq!(SandboxPolicy::app().heap_bytes, 1024 * 1024);
    }

    #[test]
    fn fuel_trap_on_burn() {
        assert!(expect_fuel_trap(), "burn loop must exhaust Frugal fuel");
    }

    #[test]
    fn fuel_is_measured_not_estimated() {
        let rt = FrugalRuntime::new();
        let r = rt
            .execute(&tiny_identity_wasm(), Grants::NONE, 1, 7)
            .unwrap();
        assert_eq!(r.value, 7);
        assert_eq!(r.fuel_limit, FRUGAL_FUEL);
        assert!(r.fuel_used > 0, "execução tem de gastar fuel: {r:?}");
        assert!(r.fuel_used < FRUGAL_FUEL);
    }

    #[test]
    fn inspection_rejects_unknown_import() {
        let rt = FrugalRuntime::new();
        let wasm = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write"
                    (func (param i32 i32 i32 i32) (result i32)))
                (func (export "run") (param i32) (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        let i = rt.inspect(&wasm).unwrap();
        assert!(!i.compatible());
        assert_eq!(i.denied_imports.len(), 1);
        assert_eq!(
            i.denied_imports[0].to_string(),
            "wasi_snapshot_preview1::fd_write"
        );
        // A execução recusa antes de instanciar (erro nomeado, fail-closed).
        let e = rt.execute(&wasm, Grants::NONE, 1, 0).unwrap_err();
        assert!(matches!(e, WasmError::ImportDenied(_)), "{e:?}");
    }

    #[test]
    fn log_is_captured_and_bounded() {
        let rt = FrugalRuntime::new();
        let wasm = wat::parse_str(
            r#"(module
                (import "aios_log" "write" (func $log (param i32 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "hello")
                (func (export "run") (param i32) (result i32)
                    (call $log (i32.const 2) (i32.const 0) (i32.const 5))))"#,
        )
        .unwrap();
        // Sem grant de log: -1 e nenhuma saída.
        let denied = rt.execute(&wasm, Grants::NONE, 1, 0).unwrap();
        assert_eq!(denied.value, -1);
        assert!(denied.output.is_empty());
        // Com grant: bytes capturados.
        let r = rt
            .execute(&wasm, Grants::new(CapBits::NONE, true), 1, 0)
            .unwrap();
        assert_eq!(r.output_text(), "hello");
        assert!(!r.output_truncated);
    }

    #[test]
    fn result_buffer_is_read_and_abi_is_detected() {
        let rt = FrugalRuntime::new();
        let wasm = echo_app_wasm();
        let i = rt.inspect(&wasm).unwrap();
        assert_eq!(i.abi, ABI_V2);
        assert!(i.has_memory);
        let r = rt.execute(&wasm, Grants::NONE, 1, 9).unwrap();
        assert_eq!(r.value, 9);
        assert_eq!(r.result_text().as_deref(), Some("{\"app\":\"echo\"}"));
        assert!(!r.result_truncated);
    }

    #[test]
    fn result_buffer_out_of_bounds_is_contract_error() {
        let rt = FrugalRuntime::new();
        let wasm = wat::parse_str(
            r#"(module
                (memory (export "memory") 1)
                (func (export "run") (param i32) (result i32) (i32.const 0))
                (func (export "result_ptr") (result i32) (i32.const 65530))
                (func (export "result_len") (result i32) (i32.const 100)))"#,
        )
        .unwrap();
        let e = rt.execute(&wasm, Grants::NONE, 1, 0).unwrap_err();
        assert!(matches!(e, WasmError::Contract(_)), "{e:?}");
    }

    #[test]
    fn module_cache_reuses_compiled_module() {
        let rt = FrugalRuntime::new();
        let wasm = tiny_identity_wasm();
        rt.execute(&wasm, Grants::NONE, 1, 1).unwrap();
        rt.execute(&wasm, Grants::NONE, 1, 2).unwrap();
        let s = rt.cache_stats();
        assert_eq!(s.misses, 1, "{s:?}");
        assert_eq!(s.hits, 1, "{s:?}");
    }

    #[test]
    fn huge_table_is_refused_by_the_limiter() {
        let rt = FrugalRuntime::new();
        let wasm = wat::parse_str(
            r#"(module
                (table 100000 funcref)
                (func (export "run") (param i32) (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        let e = rt.execute(&wasm, Grants::NONE, 1, 0).unwrap_err();
        assert!(matches!(e, WasmError::Trap(_)), "{e:?}");
    }

    #[test]
    fn oversized_module_is_refused_before_compilation() {
        let rt = FrugalRuntime::with_policy(SandboxPolicy {
            max_module_bytes: 8,
            ..SandboxPolicy::frugal()
        });
        let e = rt
            .execute(&tiny_identity_wasm(), Grants::NONE, 1, 0)
            .unwrap_err();
        assert!(matches!(e, WasmError::ModuleTooLarge(_, 8)), "{e:?}");
    }

    #[test]
    fn missing_run_is_contract_error() {
        let rt = FrugalRuntime::new();
        let wasm = wat::parse_str(r#"(module (func (export "init")))"#).unwrap();
        let e = rt.execute(&wasm, Grants::NONE, 1, 0).unwrap_err();
        assert!(matches!(e, WasmError::Contract(_)), "{e:?}");
    }

    #[test]
    fn fleet_32_identity() {
        let r = run_fleet_identity(32);
        assert_eq!(r.ok, 32, "{r:?}");
        assert_eq!(r.fuel_traps, 0);
        assert_eq!(r.heap_budget_kib, 256 * 32);
        // Bound frouxo: host Windows/WSL; só evita regressão absurda
        assert!(r.elapsed_ms < 30_000, "fleet too slow: {r:?}");
    }

    #[test]
    fn fleet_128_identity() {
        let r = run_fleet_identity(128);
        assert_eq!(r.ok, 128, "{r:?}");
        assert_eq!(r.heap_budget_kib, 256 * 128);
    }
}
