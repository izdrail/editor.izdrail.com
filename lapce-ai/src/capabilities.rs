//! Build capabilities, not claims that a particular model or GPU was tested.
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub backend: &'static str,
    pub execution: &'static str,
    pub architectures: &'static [&'static str],
    pub fim: bool,
    pub chat: bool,
    pub cuda_compiled: bool,
    pub cancellation: &'static str,
    pub sampling: &'static str,
    pub implicit_downloads: bool,
    pub limitations: &'static [&'static str],
}

/// Only backends implemented in this build are advertised. A remote server's
/// model support must be discovered there, never inferred from this registry.
pub fn available() -> [Capabilities; 2] {
    [Capabilities {
        backend: "candle", execution: "local", architectures: &["qwen2"],
        fim: true, chat: true, cuda_compiled: cfg!(feature = "candle-cuda"),
        cancellation: "cooperative between decode steps; load is not interruptible",
        sampling: "greedy", implicit_downloads: false,
        limitations: &[
            "CPU by default; explicit LAPCE_AI_DEVICE controls optional CUDA selection",
            "FIM and chat require compatible tokenizer special tokens",
            "qwen35moe target model is not supported by the current loader",
            "tokens are returned as one completion, not incremental worker events",
        ],
    }, Capabilities {
        backend: "ollama", execution: "explicit_remote", architectures: &[],
        fim: true, chat: true, cuda_compiled: false,
        cancellation: "cooperative between stream reads; blocked reads wait for timeout",
        sampling: "temperature 0.2; existing fixed lane defaults", implicit_downloads: false,
        limitations: &[
            "server hardware and model compatibility are not established by this client",
            "raw FIM requires a Qwen-compatible FIM model",
            "tokens are returned as one completion, not incremental worker events",
        ],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capabilities_do_not_claim_unimplemented_gpu_or_moe_support() {
        let providers = available();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].architectures, &["qwen2"]);
        assert_eq!(providers[1].execution, "explicit_remote");
        assert_eq!(providers[0].cuda_compiled, cfg!(feature = "candle-cuda"));
        assert!(providers.iter().all(|p| !p.implicit_downloads));
        let json = serde_json::to_value(providers).unwrap();
        assert_eq!(json[0]["backend"], "candle");
    }
}
