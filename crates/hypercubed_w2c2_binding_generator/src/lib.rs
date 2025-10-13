#![warn(clippy::all)]

use anyhow::{Context, bail, ensure};
use kdl::{KdlDocument, KdlNode};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub fn compile_document<P: AsRef<Path>, O1: AsRef<Path>, O2: AsRef<Path>>(
    doc_path: P,
    c_out_dir_path: O1,
    rust_out_dir_path: O2,
) {
    let doc_path = doc_path.as_ref();
    let c_out_dir_path = c_out_dir_path.as_ref();
    let rust_out_dir_path = rust_out_dir_path.as_ref();
    assert!(c_out_dir_path.is_dir());
    assert!(rust_out_dir_path.is_dir());
    let doc_string = std::fs::read_to_string(doc_path).unwrap();
    let out_sources = compile(&doc_string).unwrap();
    let mut rust_out_path = PathBuf::from(rust_out_dir_path);
    rust_out_path.push(doc_path.file_stem().unwrap());
    rust_out_path.set_extension("rs");
    std::fs::write(&rust_out_path, out_sources.rust).unwrap();
    let mut c_out_path = PathBuf::from(c_out_dir_path);
    c_out_path.push(doc_path.file_stem().unwrap());
    c_out_path.set_extension("c");
    std::fs::write(&c_out_path, out_sources.c).unwrap();
}

struct OutputSources {
    rust: String,
    c: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DefinedType<'a> {
    Extern {
        rust_name: &'a str,
        c_ffi_name: &'a str,
        c_name: &'a str,
    },
    ExternPtr {
        mutable: bool,
        rust_name: &'a str,
        c_name: &'a str,
    },
    Enum {
        c_conv_func_name: String,
    },
    Bitflags {
        c_conv_func_name: String,
    },
}

fn compile(doc_str: &str) -> anyhow::Result<OutputSources> {
    let doc: KdlDocument = doc_str.parse()?;
    let mut node_iter = doc.nodes().iter();
    macro_rules! span_start_line {
        ($span:expr) => {{
            let span = $span;
            let substr = &doc_str[0..span.offset()];
            substr.lines().count() + substr.ends_with('\n') as usize
        }};
    }
    // Parse the header to get the basic translation types.
    let [rust_enum_type, rust_bitflags_type] = ["u32"; 2];
    let (module_name, c_enum_type, c_bitflags_type) = {
        let header_node = node_iter
            .next()
            .context("Expected document to not be empty")?;
        ensure!(header_node.name().value() == "header");
        let child_doc = header_node
            .children()
            .context("Expected header node to have children")?;
        let mut module_name: Option<&str> = None;
        let mut c_enum_type: Option<&str> = None;
        let mut c_bitflags_type: Option<&str> = None;
        for header_field_node in child_doc.nodes() {
            match header_field_node.name().value() {
                // `module-name <Module Name: String>`
                "module-name" => {
                    ensure!(
                        module_name.is_none(),
                        "Header module name already specified",
                    );
                    let name = header_field_node
                        .get(0)
                        .context("Header must have a `module-name` field")?
                        .as_string()
                        .context("Header module name must be a string")?;
                    module_name = Some(name);
                }
                // `enum-int-type rust="u32" c-ffi="U32" c=<C Type: String>`
                "enum-int-type" => {
                    ensure!(
                        c_enum_type.is_none(),
                        "Header C enum type already specified",
                    );
                    let rust_type = header_field_node
                        .get("rust")
                        .context("Header `enum-type` must have a Rust type")?
                        .as_string()
                        .context("Header `enum-type` Rust type must be a string")?;
                    ensure!(rust_type == "u32");
                    let c_ffi_type = header_field_node
                        .get("c-ffi")
                        .context("Header `enum-type` must have a C FFI type")?
                        .as_string()
                        .context("Header `enum-type` C FFI type must be a string")?;
                    ensure!(c_ffi_type == "U32");
                    let c_type = header_field_node
                        .get("c")
                        .context("Header `enum-type` must have a C type")?
                        .as_string()
                        .context("Header `enum-type` C type must be a string")?;
                    c_enum_type = Some(c_type);
                }
                // `bitflags-int-type rust="u32" c=<C Type: String>`
                "bitflags-int-type" => {
                    ensure!(
                        c_bitflags_type.is_none(),
                        "Header C bitflags type already specified",
                    );
                    let rust_type = header_field_node
                        .get("rust")
                        .context("Header `bitflags-type` must have a Rust type")?
                        .as_string()
                        .context("Header `bitflags-type` Rust type must be a string")?;
                    ensure!(rust_type == "u32");
                    let c_ffi_type = header_field_node
                        .get("c-ffi")
                        .context("Header `bitflags-type` must have a C FFI type")?
                        .as_string()
                        .context("Header `bitflags-type` C FFI type must be a string")?;
                    ensure!(c_ffi_type == "U32");
                    let c_type = header_field_node
                        .get("c")
                        .context("Header `bitflags-type` must have a C type")?
                        .as_string()
                        .context("Header `bitflags-type` C type must be a string")?;
                    c_bitflags_type = Some(c_type);
                }
                other => bail!("Unknown header field \"{other}\""),
            }
        }
        ensure!(module_name.is_some(), "Header expected `module-name` field");
        ensure!(c_enum_type.is_some(), "Header expected `enum-type` field");
        ensure!(
            c_bitflags_type.is_some(),
            "Header expected `bitflags-type` field"
        );
        (
            module_name.unwrap(),
            c_enum_type.unwrap(),
            c_bitflags_type.unwrap(),
        )
    };
    // Read the rest of the body, translating to Rust and C code as we go.
    let mut output_rust = String::new();
    let mut output_rust_extern_fns = format!(
        "#[link(wasm_import_module = \"{module_name}\")]\n{}",
        "unsafe extern \"C\" {\n",
    );
    macro_rules! write_rust_externs {
        ($($arg:tt)*) => { write!(&mut output_rust_extern_fns, $($arg)*).unwrap() };
    }
    let mut output_c = String::from(concat!(
        "#include \"w2c2_base.h\"\n\n",
        "extern wasmMemory *wasiMemory(void *instance);\n\n",
    ));
    macro_rules! write_c {
        ($($arg:tt)*) => { write!(&mut output_c, $($arg)*).unwrap() };
    }
    let mut defined_types: HashMap<&str, DefinedType> = HashMap::new();
    for node in node_iter {
        match node.name().value() {
            // `include-c <C Code: String>`
            "include-c" => {
                let c_code = node[0].as_string().unwrap();
                write_c!("{c_code}\n\n");
            }
            // `extern-type <Name: String> \
            //     rust=<Rust Type: String> \
            //     c-ffi=<C Type: String> \
            //     c=<C Type: String>`
            "extern-type" => {
                let type_name = node
                    .get(0)
                    .context("`extern-type` must have a type name")?
                    .as_string()
                    .context("`extern-type` type name must be a string")?;
                let rust_name = node
                    .get("rust")
                    .context("`extern-type` must have a Rust type")?
                    .as_string()
                    .context("`extern-type` Rust type must be a string")?;
                let c_ffi_name = node
                    .get("c-ffi")
                    .context("`extern-type` must have a C FFI type")?
                    .as_string()
                    .context("`extern-type` C FFI type must be a string")?;
                let c_name = node
                    .get("c")
                    .context("`extern-type` must have a C type")?
                    .as_string()
                    .context("`extern-type` C type must be a string")?;
                let old_entry = defined_types.insert(
                    type_name,
                    DefinedType::Extern {
                        rust_name,
                        c_ffi_name,
                        c_name,
                    },
                );
                ensure!(
                    old_entry.is_none(),
                    "Type \"{type_name}\" should not be already defined",
                );
            }
            // `extern-ptr-type <"mut"|"const"> <Name: String> \
            //     rust=<Rust Pointee Type: String> \
            //     c=<C Pointee Type: String>`
            "extern-ptr-type" => {
                let mut_specifier_str = node
                    .get(0)
                    .context("`extern-ptr-type` must have a mutability specifier")?
                    .as_string()
                    .context("`extern-ptr-type` mutability specifier must be a string")?;
                let mutable = match mut_specifier_str {
                    "mut" => true,
                    "const" => false,
                    other => bail!(
                        "Unknown mutability specifier \"{}\", expected \"mut\" or \"const\"",
                        other
                    ),
                };
                let type_name = node
                    .get(1)
                    .context("`extern-ptr-type` must have a type name")?
                    .as_string()
                    .context("`extern-ptr-type` type name must be a string")?;
                let rust_name = node
                    .get("rust")
                    .context("`extern-ptr-type` must have a Rust type")?
                    .as_string()
                    .context("`extern-ptr-type` Rust type must be a string")?;
                let c_name = node
                    .get("c")
                    .context("`extern-ptr-type` must have a C type")?
                    .as_string()
                    .context("`extern-ptr-type` C type must be a string")?;
                let old_entry = defined_types.insert(
                    type_name,
                    DefinedType::ExternPtr {
                        mutable,
                        rust_name,
                        c_name,
                    },
                );
                ensure!(
                    old_entry.is_none(),
                    "Type \"{type_name}\" should not be already defined",
                );
            }
            // ```
            // enum <Type Name: String> {
            //     - <Field Name: String> c=<Translation C Value: String>
            //     ...
            // }
            // ```
            "enum" => translate_enum(
                module_name,
                rust_enum_type,
                c_enum_type,
                &mut output_rust,
                &mut output_c,
                &mut defined_types,
                node,
            )
            .with_context(|| {
                format!(
                    "Error while parsing enum definition at line {}",
                    span_start_line!(node.span()),
                )
            })?,
            // ```
            // bitflags <Type Name: String> {
            //     - <Field Name: String> c=<Translation C Value: String>
            //     ...
            // }
            // ```
            "bitflags" => translate_bitflags(
                module_name,
                rust_bitflags_type,
                c_bitflags_type,
                &mut output_rust,
                &mut output_c,
                &mut defined_types,
                node,
            )
            .with_context(|| {
                format!(
                    "Error while parsing bitflags definition at line {}",
                    span_start_line!(node.span()),
                )
            })?,
            // ```
            // bind-c-fn <Name: String> c=<C Bind Name: String> {
            //     args $(<Arg Name: String>=<Arg Type: String>)*
            //     $(
            //         return <Arg Type: String>
            //     )?
            //     $(
            //         // $FUNCTION and $WRAPPED function are available.
            //         rust-wrapper <Rust Code: String>
            //     )?
            // }
            // ```
            "bind-c-fn" => translate_c_binding_fn(
                module_name,
                c_enum_type,
                c_bitflags_type,
                &mut output_rust,
                &mut output_rust_extern_fns,
                &mut output_c,
                &mut defined_types,
                node,
            )
            .with_context(|| {
                format!(
                    "Error while parsing C binding function at line {}",
                    span_start_line!(node.span()),
                )
            })?,
            other => bail!("Unknown command \"{other}\""),
        }
    }
    write_rust_externs!("}}\n");
    output_rust.push_str(output_rust_extern_fns.as_str());
    Ok(OutputSources {
        rust: output_rust,
        c: output_c,
    })
}

fn translate_enum<'a>(
    module_name: &str,
    rust_enum_type: &str,
    c_enum_type: &str,
    output_rust: &mut String,
    output_c: &mut String,
    defined_types: &mut HashMap<&'a str, DefinedType<'a>>,
    node: &'a KdlNode,
) -> anyhow::Result<()> {
    macro_rules! write_rust {
        ($($arg:tt)*) => { write!(output_rust, $($arg)*).unwrap() };
    }
    macro_rules! write_c {
        ($($arg:tt)*) => { write!(output_c, $($arg)*).unwrap() };
    }
    let type_name = node
        .get(0)
        .context("Enum must have a type name")?
        .as_string()
        .context("Enum type name must be a string")?;
    let child_nodes = node.children().map_or([].as_slice(), |doc| doc.nodes());
    // Start Rust enum definition
    write_rust!("#[repr({rust_enum_type})]\n");
    write_rust!("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    write_rust!("pub enum {type_name} {{\n");
    // Start C enum definition
    write_c!("enum {type_name} {{");
    // Start C conversion function definition
    let c_function_name = format!("{module_name}_conv_enum_rs_to_c__{type_name}");
    let mut c_function = format!("static {c_enum_type} {c_function_name}(U32 value) {{\n");
    c_function.push_str("    switch (value) {\n");
    macro_rules! write_c_function {
        ($($arg:tt)*) => { write!(&mut c_function, $($arg)*).unwrap() };
    }
    for (i, field_node) in child_nodes.iter().enumerate() {
        ensure!(
            field_node.name().value() == "-",
            "Unexpected named child in enum \"{type_name}\"",
        );
        let field_name = field_node
            .get(0)
            .context("Enum field must have a name")?
            .as_string()
            .context("Enum field name must be a string")?;
        // Mangle the C field name, so we don't have any collisions.
        let c_field_name = format!("{type_name}Field{field_name}");
        let c_value = field_node
            .get("c")
            .context("Enum field must have a translated C value")?
            .as_string()
            .context("Enum field translated C value must be a string")?;
        // Add Rust enum field
        write_rust!("    {field_name} = {i},\n");
        // Add C enum field
        output_c.push_str(if i > 0 { ",\n" } else { "\n" });
        write_c!("    {c_field_name} = {i}");
        // Add C conversion case
        write_c_function!("        case {c_field_name}: return {c_value};\n");
    }
    // End Rust enum definition.
    write_rust!("}}\n\n");
    // End C enum definition.
    write_c!("\n}};\n\n");
    // End and add C conversion function.
    // Default case should be unreachable.
    write_c_function!("        default: return 0;\n");
    write_c_function!("    }}\n");
    write_c_function!("}}\n\n");
    output_c.push_str(c_function.as_str());
    let old_entry = defined_types.insert(
        type_name,
        DefinedType::Enum {
            c_conv_func_name: c_function_name,
        },
    );
    ensure!(
        old_entry.is_none(),
        "Type \"{type_name}\" should not be already defined",
    );
    Ok(())
}

fn translate_bitflags<'a>(
    module_name: &str,
    rust_bitflags_type: &str,
    c_bitflags_type: &str,
    output_rust: &mut String,
    output_c: &mut String,
    defined_types: &mut HashMap<&'a str, DefinedType<'a>>,
    node: &'a KdlNode,
) -> anyhow::Result<()> {
    macro_rules! write_rust {
        ($($arg:tt)*) => { write!(output_rust, $($arg)*).unwrap() };
    }
    macro_rules! write_c {
        ($($arg:tt)*) => { write!(output_c, $($arg)*).unwrap() };
    }
    let type_name = node
        .get(0)
        .context("Bitflags must have a type name")?
        .as_string()
        .context("Bitflags type name must be a string")?;
    let child_nodes = node.children().map_or([].as_slice(), |doc| doc.nodes());
    // Start Rust bitflags definition
    write_rust!("bitflags::bitflags! {{\n");
    write_rust!("    #[repr(transparent)]\n");
    write_rust!("    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    write_rust!("    pub struct {type_name}: {rust_bitflags_type} {{\n");
    // Start C bitflags definition
    write_c!("enum {type_name} {{");
    // Start C conversion function definition
    let c_function_name = format!("{module_name}_conv_bitflags_rs_to_c__{type_name}");
    let mut c_function = format!("static {c_bitflags_type} {c_function_name}(U32 value) {{\n");
    macro_rules! write_c_function {
        ($($arg:tt)*) => { write!(&mut c_function, $($arg)*).unwrap() };
    }
    write_c_function!("    {c_bitflags_type} out = 0;\n");
    for (i, field_node) in child_nodes.iter().enumerate() {
        ensure!(
            field_node.name().value() == "-",
            "Unexpected named child in bitflags \"{type_name}\"",
        );
        let flag_value = 1u32 << i;
        let field_name = field_node
            .get(0)
            .context("Bitflags field must have a name")?
            .as_string()
            .context("Bitflags field name must be a string")?;
        // Mangle the C field name, so we don't have any collisions.
        let c_field_name = format!("{type_name}Field{field_name}");
        let c_value = field_node
            .get("c")
            .context("Bitflags field must have a translated C value")?
            .as_string()
            .context("Bitflags field translated C value must be a string")?;
        // Add Rust bitflags field
        write_rust!("        const {field_name} = {flag_value:#010X};\n");
        // Add C bitflags field
        output_c.push_str(if i > 0 { ",\n" } else { "\n" });
        write_c!("    {c_field_name} = {flag_value:#010X}");
        // Add C conversion case
        write_c_function!("    if (value & {c_field_name} != 0) out |= {c_value};\n");
    }
    // End Rust bitflags definition.
    write_rust!("    }}\n");
    write_rust!("}}\n\n");
    // End C bitflags definition.
    write_c!("\n}};\n\n");
    // End and add C conversion function.
    write_c_function!("    return out;\n");
    write_c_function!("}}\n\n");
    output_c.push_str(c_function.as_str());
    let old_entry = defined_types.insert(
        type_name,
        DefinedType::Bitflags {
            c_conv_func_name: c_function_name,
        },
    );
    ensure!(
        old_entry.is_none(),
        "Type \"{type_name}\" should not be already defined",
    );
    Ok(())
}

fn translate_c_binding_fn<'a>(
    module_name: &str,
    c_enum_type: &str,
    c_bitflags_type: &str,
    output_rust: &mut String,
    output_rust_extern_fns: &mut String,
    output_c: &mut String,
    defined_types: &mut HashMap<&'a str, DefinedType<'a>>,
    node: &'a KdlNode,
) -> anyhow::Result<()> {
    macro_rules! write_rust {
        ($($arg:tt)*) => { write!(output_rust, $($arg)*).unwrap() };
    }
    macro_rules! write_rust_externs {
        ($($arg:tt)*) => { write!(output_rust_extern_fns, $($arg)*).unwrap() };
    }
    macro_rules! write_c {
        ($($arg:tt)*) => { write!(output_c, $($arg)*).unwrap() };
    }
    let fn_name = node
        .get(0)
        .context("Binding must have a name")?
        .as_string()
        .context("Binding name must be a string")?;
    let bound_c_fn_name = node
        .get("c")
        .with_context(|| format!("Binding \"{fn_name}\" must have a C function to bind to"))?
        .as_string()
        .with_context(|| {
            format!("Binding \"{fn_name}\" bound C function name1 must be a string")
        })?;
    let child_doc = node
        .children()
        .with_context(|| format!("Binding \"{fn_name}\" must have a body"))?;
    // Get arguments.
    let args_node = child_doc
        .get("args")
        .with_context(|| format!("Binding \"{fn_name}\" must have an arguments field"))?;
    #[derive(Clone, Copy, Debug)]
    struct Argument<'a> {
        pub name: &'a str,
        pub type_name: &'a str,
        pub ty: &'a DefinedType<'a>,
    }
    let mut arguments: Vec<Argument> = Vec::with_capacity(args_node.entries().len());
    for entry in args_node.entries() {
        let arg_name = entry
            .name()
            .with_context(|| format!("C function binding \"{fn_name}\" has invalid argument",))?
            .value();
        ensure!(arg_name != "raw_w2c2_instance");
        ensure!(arg_name != "w2c2_memory_data");
        let arg_type_name = entry.value().as_string().with_context(|| {
            format!(
                "C function binding \"{}\" argument \"{}\" must have a string value",
                fn_name, arg_name,
            )
        })?;
        let arg_type = defined_types.get(arg_type_name).with_context(|| {
            format!(
                "C function binding \"{}\" argument \"{}\" has unknown type \"{}\"",
                fn_name, arg_name, arg_type_name,
            )
        })?;
        arguments.push(Argument {
            name: arg_name,
            type_name: arg_type_name,
            ty: arg_type,
        });
    }
    // Get return type, if provided.
    let return_type = match child_doc.get_arg("return") {
        None => None,
        Some(value) => {
            let return_type_name = value
                .as_string()
                .with_context(|| format!("Binding \"{fn_name}\" return type must be a string"))?;
            let return_type = defined_types.get(return_type_name).with_context(|| {
                format!("Binding \"{fn_name}\" returns unknown type \"{return_type_name}\"",)
            })?;
            Some(return_type)
        }
    };
    // Get Rust wrapper function, if provided.
    let (rust_wrapper, rust_extern_name) = match child_doc.get_arg("rust-wrapper") {
        None => (None, fn_name.to_owned()),
        Some(value) => {
            let wrapped_fn_name = format!("{fn_name}_raw_extern");
            let rust_wrapper = value
                .as_string()
                .with_context(|| {
                    format!("Binding \"{fn_name}\" Rust wrapper code must be a string")
                })?
                .replace("$FUNCTION", fn_name)
                .replace("$WRAPPED_FUNCTION", &wrapped_fn_name);
            (Some(rust_wrapper), wrapped_fn_name)
        }
    };
    // Write Rust code.
    if let Some(wrapper_code) = rust_wrapper {
        write_rust!("{wrapper_code}\n\n");
        write_rust_externs!("    #[link_name = \"{fn_name}\"]\n");
        write_rust_externs!("    ");
    } else {
        write_rust_externs!("    pub ");
    }
    write_rust_externs!("unsafe fn {rust_extern_name}(\n");
    for &arg in &arguments {
        let type_name = match arg.ty {
            DefinedType::Extern { rust_name, .. } => *rust_name,
            DefinedType::ExternPtr {
                mutable, rust_name, ..
            } => {
                write_rust_externs!(
                    "        {}: *{} {},\n",
                    arg.name,
                    if *mutable { "mut" } else { "const" },
                    rust_name,
                );
                continue;
            }
            _ => arg.type_name,
        };
        write_rust_externs!("        {}: {},\n", arg.name, type_name);
    }
    write_rust_externs!("    );\n");
    // Write C code.
    let c_return_type = match return_type {
        None => "void",
        Some(DefinedType::Extern { c_name, .. }) => *c_name,
        Some(_) => bail!("Non-extern return types are unimplemented"),
    };
    write_c!("{c_return_type} {module_name}__{fn_name}(\n");
    write_c!("    void *raw_w2c2_instance");
    for &arg in &arguments {
        let type_name = match arg.ty {
            DefinedType::Extern { c_ffi_name, .. } => *c_ffi_name,
            DefinedType::ExternPtr { .. } => "U32",
            DefinedType::Enum { .. } => "U32",
            DefinedType::Bitflags { .. } => "U32",
        };
        write_c!(",\n    {} {}", type_name, arg.name);
    }
    write_c!("\n) {{\n");
    write_c!("    void *w2c2_memory_data = (void *)wasiMemory(raw_w2c2_instance)->data;\n");
    write_c!("    return {bound_c_fn_name}(");
    for (i, &arg) in arguments.iter().enumerate() {
        write_c!("{}\n", if i == 0 { "" } else { "," });
        match arg.ty {
            DefinedType::Extern { c_name, .. } => write_c!("        ({}){}", c_name, arg.name,),
            DefinedType::ExternPtr {
                mutable, c_name, ..
            } => write_c!(
                "        ({}{} *)(w2c2_memory_data + {})",
                if *mutable { "" } else { "const " },
                c_name,
                arg.name,
            ),
            DefinedType::Enum {
                c_conv_func_name, ..
            } => write_c!("        {}({})", c_conv_func_name, arg.name,),
            DefinedType::Bitflags {
                c_conv_func_name, ..
            } => write_c!("        {}({})", c_conv_func_name, arg.name,),
        }
    }
    write_c!("\n    );\n");
    write_c!("}}\n\n");
    Ok(())
}
