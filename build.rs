fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_FEATURE_USE_EMBEDDED_CACHE").is_ok() {
        let embedded_cache = if std::env::var("CARGO_FEATURE_EMBED_VANILLA_CACHE").is_ok() {
            resources::block::generate_vanilla_embedded_cache().unwrap()
        } else {
            panic!(concat!(
                "Feature `use_embedded_cache` requires another feature to be enabled to ",
                "specify which cache to generate and embed. ",
                "See feature `embed_vanilla_cache`.",
            ));
        };
        let out_dir = std::env::var("OUT_DIR").unwrap();
        // XXX: DEBUG
        {
            bincode_encode_to_file(
                &embedded_cache.block_registry,
                format!("{out_dir}/block_registry.bincode"),
            );
            bincode_encode_to_file(
                &embedded_cache.models,
                format!("{out_dir}/models.bincode"),
            );
            // XXX: DEBUG
            {
                bincode_encode_to_file(
                    &embedded_cache.models.model_list,
                    format!("{out_dir}/models_model_list.bincode"),
                );
                bincode_encode_to_file(
                    &embedded_cache.models.custom_block_faces,
                    format!("{out_dir}/models_custom_block_faces.bincode"),
                );
                println!(
                    "cargo::warning=Number of Custom Block Faces: {}",
                    embedded_cache.models.custom_block_faces.len(),
                );
                println!(
                    "cargo::warning=dbg: {}",
                    std::mem::size_of::<resources::block::model::ModelVertex>(),
                );
            }
            bincode_encode_to_file(
                &embedded_cache.atlas,
                format!("{out_dir}/atlas.bincode"),
            );
            for (i, model) in embedded_cache.models.model_list.iter().enumerate() {
                bincode_encode_to_file(
                    model,
                    format!("{out_dir}/models/{i}.bincode"),
                );
            }
        }
        bincode_encode_to_file(
            embedded_cache,
            format!("{out_dir}/embedded_cache.bincode"),
        );
    }
}

fn bincode_encode_to_file<P: AsRef<std::path::Path>, E: bincode::Encode>(
    value: E,
    path: P,
) {
    let bytes = bincode::encode_to_vec(value, bincode::config::standard()).unwrap();
    if bytes.len() < 64 {
        return;
    }
    std::fs::write(path, bytes).unwrap();
}
