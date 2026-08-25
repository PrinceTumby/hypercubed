fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // Generate and serialise vanilla block definitions.
    let registrations_list = resources_vanilla_data_gen::blocks::registrations();
    let bytes = postcard::to_stdvec(&registrations_list).unwrap();
    let compressed_bytes = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
    std::fs::write(format!("{out_dir}/blocks_registration_list.postcard.zlib"), compressed_bytes).unwrap();
}
