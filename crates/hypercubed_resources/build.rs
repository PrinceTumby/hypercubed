use nickel_lang_core::error::IntoDiagnostics;
use nickel_lang_core::eval::cache::lazy::CBNCache;
use nickel_lang_core::program::Program as NickelProgram;
use std::io::Write;

#[derive(Clone, Copy, Debug)]
struct NickelTraceWriter;

impl Write for NickelTraceWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let trace_str = std::str::from_utf8(buf).unwrap();
        println!("cargo::warning={trace_str}");
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct NickelReporter;

type NickelWarning = (
    nickel_lang_core::error::warning::Warning,
    nickel_lang_core::files::Files,
);

impl nickel_lang_core::error::Reporter<NickelWarning> for NickelReporter {
    fn report(&mut self, (warning, mut files): NickelWarning) {
        for diagnostic in warning.into_diagnostics(&mut files) {
            println!("cargo::warning={diagnostic:?}");
        }
    }
}

fn main() {
    println!("cargo::rerun-if-changed=src/block/vanilla_blocks.ncl");
    type CBNCachedProgram = NickelProgram<CBNCache>;
    let mut program = CBNCachedProgram::new_from_file(
        "src/block/vanilla_blocks.ncl",
        NickelTraceWriter,
        NickelReporter,
    )
    .unwrap();
    let rich_term = program.eval_full_for_export().unwrap();
    let exported_json = nickel_lang_core::serialize::to_string(
        nickel_lang_core::serialize::ExportFormat::Json,
        &rich_term,
    )
    .unwrap();
    // At the time of writing, compression takes the file size down from 1.22MB to just 26KB, which
    // given the file gets embedded into the final binary, this reduces the binary size by a pretty
    // decent amount.
    // Ideally we'd store a compressed version using a file format that isn't JSON, but it's
    // currently the fastest self-describing format for this kind of data (mostly just repeated
    // strings).
    // Any big improvement would have to come from parsing the JSON data here into a
    // `Vec<Registration>`, and then saving that using something like postcard.
    // Unfortunately that would require moving the block registration types into yet another crate,
    // which I don't think is worth the extra annoyance.
    let compressed_json = miniz_oxide::deflate::compress_to_vec_zlib(exported_json.as_bytes(), 9);
    std::fs::write(
        format!(
            "{}/vanilla_blocks_generated.json.zlib",
            std::env::var("OUT_DIR").unwrap()
        ),
        &compressed_json,
    )
    .unwrap();
}
