//! Audited immutable Qwen2 artifacts. Memory figures are planning estimates, not guarantees.
pub struct ModelEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub chat: bool,
    pub repo: &'static str,
    pub revision: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub hash: &'static str,
}
pub const MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "qwen-fim-1.5b",
        label: "FIM: Qwen Coder 1.5B | 986 MB | allow 4 GB free RAM",
        chat: false,
        repo: "QuantFactory/Qwen2.5-Coder-1.5B-GGUF",
        revision: crate::MODEL_REV,
        file: crate::MODEL_FILE,
        size: 986048352,
        hash: crate::MODEL_HASH,
    },
    ModelEntry {
        id: "qwen-fim-3b",
        label: "FIM: Qwen Coder 3B | 1.93 GB | allow 6 GB free RAM",
        chat: false,
        repo: "bartowski/Qwen2.5-Coder-3B-GGUF",
        revision: "465c183318f1fcb5774394eee76f1b7f224494ec",
        file: "Qwen2.5-Coder-3B-Q4_K_M.gguf",
        size: 1929903200,
        hash: "bf2cee2051affae926e9a9ef4a62ebd197aec86639b075634faf973fcfd8a10c",
    },
    ModelEntry {
        id: "qwen-chat-0.5b",
        label: "Chat: Qwen Coder 0.5B Instruct | 491 MB | allow 3 GB free RAM",
        chat: true,
        repo: "QuantFactory/Qwen2.5-Coder-0.5B-Instruct-GGUF",
        revision: "e91d128ba1c331fdec779e0ff21efa7568274e08",
        file: "Qwen2.5-Coder-0.5B-Instruct.Q4_K_M.gguf",
        size: 491400512,
        hash: "5d933e04310b9184f9bc97e511820e71b5dc7704eda6f7c937126a241828e93e",
    },
    ModelEntry {
        id: "qwen-chat-1.5b",
        label: "Chat: Qwen Coder 1.5B Instruct | 986 MB | allow 4 GB free RAM",
        chat: true,
        repo: "QuantFactory/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        revision: "0a037c7a9331fd594a5eff5f977941ef4f9a092b",
        file: "Qwen2.5-Coder-1.5B-Instruct.Q4_K_M.gguf",
        size: 986048480,
        hash: "ef9e3d005b760b65cef130499ab12389d3e09c56861aad0e3d6234846c5be957",
    },
    ModelEntry {
        id: "qwen-chat-3b",
        label: "Chat: Qwen Coder 3B Instruct | 2.10 GB | allow 6 GB free RAM",
        chat: true,
        repo: "QuantFactory/Qwen2.5-Coder-3B-Instruct-GGUF",
        revision: "c2a97103d73fd95a128e6632ad2222e05042eb0c",
        file: "Qwen2.5-Coder-3B-Instruct.Q4_K_M.gguf",
        size: 2104933312,
        hash: "be945f616db40778ce845eb0ffaff30f98dec2eb2c43ba288996e98f1171348e",
    },
];
pub fn find(id: &str) -> Option<&'static ModelEntry> {
    MODELS.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_pinned_and_typed() {
        for m in MODELS {
            assert_eq!(m.revision.len(), 40);
            assert_eq!(m.hash.len(), 64);
            assert!(m.hash.bytes().all(|c| c.is_ascii_hexdigit()));
            assert!(m.size > 400_000_000);
            assert_eq!(m.chat, m.file.contains("Instruct"));
            assert!(find(m.id).is_some());
        }
        assert!(find("qwen-chat-1.5b").unwrap().chat);
        assert!(!find("qwen-fim-1.5b").unwrap().chat);
    }
}
