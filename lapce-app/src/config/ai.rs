use serde::{Deserialize, Serialize};
use structdesc::FieldNames;
#[derive(FieldNames, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AiConfig {
    #[field_names(
        desc = "Enable local manual AI ghost text. No code is sent to the network."
    )]
    pub enabled: bool,
    #[field_names(
        desc = "Local Qwen2 GGUF path; empty uses the downloaded curated model."
    )]
    pub model_path: String,
    #[field_names(
        desc = "Matching tokenizer.json path; empty uses the downloaded tokenizer."
    )]
    pub tokenizer_path: String,
    #[field_names(desc = "Prefix token budget (1-2048).")]
    pub prefix_tokens: usize,
    #[field_names(desc = "Suffix token budget (0-1024).")]
    pub suffix_tokens: usize,
    #[field_names(desc = "Output token limit (1-128).")]
    pub max_tokens: usize,
}
impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_path: String::new(),
            tokenizer_path: String::new(),
            prefix_tokens: 512,
            suffix_tokens: 128,
            max_tokens: 32,
        }
    }
}
