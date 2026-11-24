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
pub fn available() -> Vec<Capabilities> {
    #[allow(unused_mut)]
    let mut providers = vec![Capabilities {
        backend: "candle", execution: "local", architectures: &["qwen2"],
        fim: true, chat: true, cuda_compiled: cfg!(feature = "candle-cuda"),
        cancellation: "cooperative between decode steps; load is not interruptible",
        sampling: "greedy default; explicit Candle chat/FIM profiles support temperature/top-k/top-p/seed/output penalties/stop IDs", implicit_downloads: false,
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
    }, Capabilities {
        backend: "mistral.rs", execution: "explicit_local_worker", architectures: &[],
        fim: false, chat: true, cuda_compiled: false,
        cancellation: "hard process cancellation unloads model; cooperative SDK cancellation not implemented",
        sampling: "greedy deterministic", implicit_downloads: false,
        limitations: &[
            "requires separately built explicit local worker executable",
            "worker CPU build passed; exact-model inference and CUDA remain unverified",
            "CUDA build state belongs to the separate worker, not this executable",
            "raw FIM unsupported; chat requires local model auxiliary files",
        ],
    }];
    #[cfg(feature = "llama-cpp")]
    providers.push(Capabilities {
        backend: "llama.cpp", execution: "explicit_local", architectures: &[],
        fim: true, chat: true, cuda_compiled: cfg!(feature = "llama-cpp-cuda"),
        cancellation: "cooperative between prefill batches and decode steps; load is not interruptible",
        sampling: "greedy", implicit_downloads: false,
        limitations: &[
            "experimental: CPU build passed; exact-model inference acceptance pending",
            "FIM requires special tokens; chat requires supported embedded template",
            "GPU layer and CPU expert settings request placement, not proof of CUDA inference",
            "tokens returned as one completion, not incremental worker events",
        ],
    });
    providers
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capabilities_do_not_claim_unimplemented_gpu_or_moe_support() {
        let providers = available();
        assert_eq!(providers.len(), if cfg!(feature = "llama-cpp") {4} else {3});
        assert_eq!(providers[0].architectures, &["qwen2"]);
        assert_eq!(providers[1].execution, "explicit_remote");
        assert_eq!(providers[0].cuda_compiled, cfg!(feature = "candle-cuda"));
        assert!(providers.iter().all(|p| !p.implicit_downloads));
        let json = serde_json::to_value(providers).unwrap();
        assert_eq!(json[0]["backend"], "candle");
    }
}
