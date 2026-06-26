use std::{path::PathBuf, sync::Arc};

use candle_transformers::models::t5;

use candle_core::Device;
use llama_rs::{
    GgufFile, InferenceContext, Model, Sampler, Tokenizer, backend::cpu, default_backend,
    load_llama_model,
};

struct T5ModelBuilder {
    device: Device,
    config: t5::Config,
    weights_filename: Vec<PathBuf>,
}

impl T5ModelBuilder {
    pub fn load() -> anyhow::Result<(Self, Tokenizer)> {
        let device = Device::Cpu;
        // let (default_model, default_revision) = match args.which {
        //     Which::T5Base => ("t5-base", "main"),
        //     Which::T5Small => ("t5-small", "refs/pr/15"),
        //     Which::T5Large => ("t5-large", "main"),
        //     Which::T5_3B => ("t5-3b", "main"),
        //     Which::Mt5Base => ("google/mt5-base", "refs/pr/5"),
        //     Which::Mt5Small => ("google/mt5-small", "refs/pr/6"),
        //     Which::Mt5Large => ("google/mt5-large", "refs/pr/2"),
        // };
        // let default_model = "t5-base";
        // let default_revision = "main";
        // let default_model = default_model.to_string();
        // let default_revision = default_revision.to_string();
        // let (model_id, revision) = match (args.model_id.to_owned(), args.revision.to_owned()) {
        //     (Some(model_id), Some(revision)) => (model_id, revision),
        //     (Some(model_id), None) => (model_id, "main".to_string()),
        //     (None, Some(revision)) => (default_model, revision),
        //     (None, None) => (default_model, default_revision),
        // };

        // let repo = Repo::with_revision(model_id.clone(), RepoType::Model, revision);
        // let api = Api::new()?;
        // let repo = api.repo(repo);
        // let config_filename = match &args.config_file {
        //     None => repo.get("config.json")?,
        //     Some(f) => f.into(),
        // };
        // let tokenizer_filename = match &args.tokenizer_file {
        //     None => match args.which {
        //         Which::Mt5Base => api
        //             .model("lmz/mt5-tokenizers".into())
        //             .get("mt5-base.tokenizer.json")?,
        //         Which::Mt5Small => api
        //             .model("lmz/mt5-tokenizers".into())
        //             .get("mt5-small.tokenizer.json")?,
        //         Which::Mt5Large => api
        //             .model("lmz/mt5-tokenizers".into())
        //             .get("mt5-large.tokenizer.json")?,
        //         _ => repo.get("tokenizer.json")?,
        //     },
        //     Some(f) => f.into(),
        // };
        // let weights_filename = match &args.model_file {
        //     Some(f) => f.split(',').map(|v| v.into()).collect::<Vec<_>>(),
        //     None => {
        //         if model_id == "google/flan-t5-xxl" || model_id == "google/flan-ul2" {
        //             candle_examples::hub_load_safetensors(&repo, "model.safetensors.index.json")?
        //         } else {
        //             vec![repo.get("model.safetensors")?]
        //         }
        //     }
        // };
        // let config = std::fs::read_to_string(config_filename)?;
        let mut config: t5::Config = t5::Config {
            vocab_size: todo!(),
            d_model: todo!(),
            d_kv: todo!(),
            d_ff: todo!(),
            num_layers: todo!(),
            num_decoder_layers: todo!(),
            num_heads: todo!(),
            relative_attention_num_buckets: todo!(),
            relative_attention_max_distance: todo!(),
            dropout_rate: todo!(),
            layer_norm_epsilon: todo!(),
            initializer_factor: todo!(),
            feed_forward_proj: todo!(),
            tie_word_embeddings: todo!(),
            is_decoder: todo!(),
            is_encoder_decoder: todo!(),
            use_cache: true,
            pad_token_id: todo!(),
            eos_token_id: todo!(),
            decoder_start_token_id: todo!(),
        };
        let tokenizer = Tokenizer::from_hf_json_str("")?;
        Ok((
            Self {
                device,
                config,
                weights_filename,
            },
            tokenizer,
        ))
    }

    pub fn build_encoder(&self) -> anyhow::Result<t5::T5EncoderModel> {
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&self.weights_filename, DTYPE, &self.device)?
        };
        Ok(t5::T5EncoderModel::load(vb, &self.config)?)
    }

    pub fn build_conditional_generation(&self) -> anyhow::Result<t5::T5ForConditionalGeneration> {
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&self.weights_filename, DTYPE, &self.device)?
        };
        Ok(t5::T5ForConditionalGeneration::load(vb, &self.config)?)
    }
}

pub fn init(path: &str) -> anyhow::Result<()> {
    Ok(())
}

// pub fn init(path: &str) -> anyhow::Result<()> {
//     let model = load_llama_model(path)?;
//     let gguf = GgufFile::open(path)?;
//     let tokenizer = Tokenizer::from_gguf(&gguf)?;

//     let backend = Arc::new(cpu::CpuBackend::new());

//     let mut ctx = InferenceContext::new(model.config(), backend);
//     let mut sampler = Sampler::new(
//         llama_rs::SamplerConfig {
//             ..Default::default()
//         },
//         1,
//     ); // temperature, top_k, top_p

//     // Encode prompt
//     let tokens = tokenizer.encode("Hello, world!", true)?;
//     // Generate
//     let mut output_tokens = tokens.clone();
//     for _ in 0..50 {
//         let logits = model.forward(&output_tokens[output_tokens.len() - 1..], &mut ctx)?;
//         let next_token = sampler.sample(&logits, &output_tokens);
//         output_tokens.push(next_token);

//         // Decode and print
//         if let Ok(text) = tokenizer.decode(&[next_token]) {
//             print!("{}", text);
//         }
//     }
//     Ok(())
// }
