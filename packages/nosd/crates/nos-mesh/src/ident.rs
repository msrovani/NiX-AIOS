//! Identidade de nó (M6) — Ed25519 + pinning/allowlist + anti-replay.
//!
//! O id curto (u32) do wire é **derivado** da chave pública (fnv-1a64), não
//! escolhido pelo operador. A autorização vem da assinatura + allowlist, nunca
//! do `id` sozinho.
//!
//! Wire do HB assinado (ver `lib.rs`):
//! `HB\0 id:u32 seq:u64 PK\0 CAP\0 cpu:u32 mem:u32 net:u32 KEY\0 pk:32B SIG\0 sig:64B`

use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::mesh_knowledge::fnv1a64;

pub const SIG_MARK: &[u8] = b"SIG\0";
pub const KEY_MARK: &[u8] = b"KEY\0";

/// Id curto derivado da chave pública (não é segredo nem commitment).
pub fn short_id(pk: &[u8]) -> u32 {
    (fnv1a64(pk) & 0xffff_ffff) as u32
}

#[derive(Clone)]
pub struct Identity {
    kp: Arc<KeyPair>,
    allow: Arc<HashSet<u32>>,
    require_sig: bool,
}

impl Identity {
    pub fn from_seed(seed: [u8; 32], allow: HashSet<u32>, require_sig: bool) -> Self {
        Self {
            kp: Arc::new(KeyPair::from_seed(Seed::new(seed))),
            allow: Arc::new(allow),
            require_sig,
        }
    }

    /// Carrega a seed (32 B) ou cria uma nova com modo 0600.
    pub fn load_or_create(
        path: &Path,
        allow: HashSet<u32>,
        require_sig: bool,
    ) -> std::io::Result<Self> {
        let seed = match std::fs::read(path) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut s = [0u8; 32];
                s.copy_from_slice(&bytes);
                s
            }
            _ => {
                let seed = Seed::generate();
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                use std::io::Write;
                let mut opts = std::fs::OpenOptions::new();
                opts.create(true).write(true).truncate(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    opts.mode(0o600);
                }
                let mut f = opts.open(path)?;
                f.write_all(seed.as_ref())?;
                f.sync_all()?;
                let mut s = [0u8; 32];
                s.copy_from_slice(seed.as_ref());
                s
            }
        };
        Ok(Self::from_seed(seed, allow, require_sig))
    }

    pub fn public_key(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(self.kp.pk.as_ref());
        out
    }

    pub fn node_id(&self) -> u32 {
        short_id(&self.public_key())
    }

    pub fn require_sig(&self) -> bool {
        self.require_sig
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        let sig = self.kp.sk.sign(msg, None);
        let mut out = [0u8; 64];
        out.copy_from_slice(sig.as_ref());
        out
    }

    /// `true` se o id está autorizado (allowlist vazia = aceitar qualquer id
    /// **com assinatura válida**, modo TOFU explícito).
    pub fn allow(&self, id: u32) -> bool {
        self.allow.is_empty() || self.allow.contains(&id)
    }

    pub fn allow_len(&self) -> usize {
        self.allow.len()
    }
}

/// Pinagem TOFU + anti-replay por peer.
#[derive(Default)]
pub struct Pinned {
    inner: Mutex<HashMap<u32, Pin>>,
}

struct Pin {
    pk: [u8; 32],
    last_seq: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PinError {
    /// Chave pública diferente da primeira vista (possível MITM/troca).
    KeyChanged,
    /// seq não monotónico (replay ou reordenação antiga).
    Replay,
    /// Id não corresponde à chave pública.
    IdMismatch,
}

impl Pinned {
    pub fn new() -> Self {
        Self::default()
    }

    /// Verifica e fixa. Primeira observação = TOFU (se a allowlist aceitar).
    pub fn check(&self, id: u32, pk: &[u8], seq: u64) -> Result<(), PinError> {
        if short_id(pk) != id {
            return Err(PinError::IdMismatch);
        }
        let mut g = self.inner.lock();
        match g.get_mut(&id) {
            Some(pin) => {
                if pin.pk != pk {
                    return Err(PinError::KeyChanged);
                }
                if seq <= pin.last_seq {
                    return Err(PinError::Replay);
                }
                pin.last_seq = seq;
                Ok(())
            }
            None => {
                let mut p = [0u8; 32];
                p.copy_from_slice(pk);
                g.insert(
                    id,
                    Pin {
                        pk: p,
                        last_seq: seq,
                    },
                );
                Ok(())
            }
        }
    }

    pub fn peer_count(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn forget(&self, id: u32) {
        self.inner.lock().remove(&id);
    }
}

/// Verifica a assinatura de um bloco `msg` com a chave `pk`.
pub fn verify(pk: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    let Ok(pk) = PublicKey::from_slice(pk) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(sig) else {
        return false;
    };
    pk.verify(msg, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(seed: u8) -> Identity {
        Identity::from_seed([seed; 32], HashSet::new(), true)
    }

    #[test]
    fn id_is_derived_from_key() {
        let a = ident(1);
        let b = ident(2);
        assert_eq!(a.node_id(), short_id(&a.public_key()));
        assert_ne!(a.node_id(), b.node_id());
    }

    #[test]
    fn sign_verify_roundtrip_and_tamper() {
        let a = ident(3);
        let msg = b"hb-payload";
        let sig = a.sign(msg);
        assert!(verify(&a.public_key(), msg, &sig));
        assert!(!verify(&a.public_key(), b"hb-payload!", &sig));
        let mut bad = sig;
        bad[0] ^= 0xff;
        assert!(!verify(&a.public_key(), msg, &bad));
    }

    #[test]
    fn pinning_rejects_key_change_and_replay() {
        let a = ident(4);
        let pk = a.public_key();
        let id = a.node_id();
        let p = Pinned::new();
        assert_eq!(p.check(id, &pk, 10), Ok(()));
        assert_eq!(p.check(id, &pk, 11), Ok(()));
        assert_eq!(p.check(id, &pk, 11), Err(PinError::Replay));
        assert_eq!(p.check(id, &pk, 9), Err(PinError::Replay));

        // O id e derivado da chave: outra chave com o mesmo id e impossivel
        // sem colisao -> IdMismatch (KeyChanged fica como defesa em profundidade).
        let evil = ident(5);
        assert_eq!(
            p.check(id, &evil.public_key(), 12),
            Err(PinError::IdMismatch)
        );
        assert_eq!(p.check(id + 1, &pk, 1), Err(PinError::IdMismatch));
    }

    #[test]
    fn allowlist_filters_ids() {
        let a = ident(6);
        let allowed: HashSet<u32> = [a.node_id()].into_iter().collect();
        let i = Identity::from_seed([6; 32], allowed, true);
        assert!(i.allow(a.node_id()));
        assert!(!i.allow(12345));
        // vazia = TOFU (aceita qualquer id com assinatura valida)
        assert!(ident(7).allow(999));
    }
}
