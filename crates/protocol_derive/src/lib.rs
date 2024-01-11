use darling::FromDeriveInput;
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields};

#[proc_macro_derive(Serialize)]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident: name,
        data,
        generics,
        ..
    } = parse_macro_input!(input);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let implementations = match data {
        Data::Struct(struct_data) => match struct_data.fields {
            Fields::Named(fields) => {
                let idents_to = fields.named.iter().map(|Field { ident, .. }| ident);
                let idents_into = idents_to.clone();
                quote! {
                    fn serialize_to<W: ::std::io::Write>(&self, writer: &mut W)
                        -> ::std::io::Result<()>
                    {
                        #(
                            self.#idents_to.serialize_to(writer)?;
                        )*
                        Ok(())
                    }

                    fn serialize_into<W: ::std::io::Write>(self, writer: &mut W)
                        -> ::std::io::Result<()>
                    {
                        #(
                            self.#idents_into.serialize_into(writer)?;
                        )*
                        Ok(())
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let field_indexes_to = (0..fields.unnamed.len()).map(syn::Index::from);
                let field_indexes_into = field_indexes_to.clone();
                quote! {
                    fn serialize_to<W: ::std::io::Write>(&self, writer: &mut W)
                        -> ::std::io::Result<()>
                    {
                        #(
                            self.#field_indexes_to.serialize_to(writer)?;
                        )*
                        Ok(())
                    }

                    fn serialize_into<W: ::std::io::Write>(self, writer: &mut W)
                        -> ::std::io::Result<()>
                    {
                        #(
                            self.#field_indexes_into.serialize_into(writer)?;
                        )*
                        Ok(())
                    }
                }
            }
            Fields::Unit => quote! {
                fn serialize_to<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::io::Result<()> {
                    Ok(())
                }
            },
        },
        _ => panic!("`Serialize` can only be derived for structs"),
    };
    let output = quote! {
        impl #impl_generics Serialize for #name #ty_generics #where_clause {
            #implementations
        }
    };
    output.into()
}

#[proc_macro_derive(Deserialize)]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident: name,
        data,
        generics,
        ..
    } = parse_macro_input!(input);
    let name_string = name.to_string();
    let implementation = match data {
        Data::Struct(struct_data) => match struct_data.fields {
            Fields::Named(fields) => {
                // `quote` doesn't insert ending commas, so we insert them manually to ensure
                // single item tuples are interpreted as tuples, not just an expression in brackets
                let field_deserializers: Vec<_> = fields
                    .named
                    .iter()
                    .map(|field| {
                        let ty = &field.ty;
                        quote! { <#ty>::deserialize, }
                    })
                    .collect();
                let idents: Vec<_> = fields
                    .named
                    .iter()
                    .map(|Field { ident, .. }| quote! { #ident, })
                    .collect();
                quote! {
                    use ::nom_supreme::parser_ext::ParserExt;
                    ::nom::sequence::tuple(( #(#field_deserializers)* ))
                        .context(#name_string)
                        .map(|( #(#idents)* )| #name { #(#idents)* })
                        .parse(input)
                }
            }
            Fields::Unnamed(fields) => {
                let field_types: Vec<_> = fields
                    .unnamed
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                match field_types.as_slice() {
                    [] => panic!("`Deserialize` cannot be derived for tuple structs with 0 fields"),
                    [field_type] => quote! {
                        <#field_type>::deserialize.map(#name).parse(input)
                    },
                    _ => {
                        let field_deserializers = field_types
                            .iter()
                            .map(|field| quote! { <#field>::deserialize, });
                        quote! {
                            use ::nom_supreme::parser_ext::ParserExt;
                            ::nom::sequence::tuple(( #(#field_deserializers)* ))
                                .context(#name_string)
                                .map(#name)
                                .parse(input)
                        }
                    }
                }
            }
            Fields::Unit => panic!("`Deserialize` cannot be derived for unit structs"),
        },
        _ => panic!("`Deserialize` can only be derived for structs"),
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let output = quote! {
        impl #impl_generics Deserialize for #name #ty_generics #where_clause {
            fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
                use ::nom::Parser;
                #implementation
            }
        }
    };
    output.into()
}

#[derive(FromDeriveInput)]
#[darling(attributes(packet_write))]
struct PacketWriteOpts {
    id: i32,
}

#[proc_macro_derive(PacketWrite, attributes(packet_write))]
pub fn derive_packet_write(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let PacketWriteOpts { id } =
        PacketWriteOpts::from_derive_input(&input).expect("Invalid options");
    let DeriveInput {
        ident, generics, ..
    } = input;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let output = quote! {
        impl #impl_generics PacketWrite for #ident #ty_generics #where_clause {
            const ID: i32 = #id;
        }
    };
    output.into()
}

#[derive(FromDeriveInput, Default)]
#[darling(default, attributes(packet_read))]
struct PacketReadOpts {
    id: Option<i32>,
}

#[proc_macro_derive(PacketRead, attributes(packet_read))]
pub fn derive_packet_read(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let PacketReadOpts { id } =
        PacketReadOpts::from_derive_input(&input).expect("Invalid options");
    let DeriveInput {
        ident, generics, ..
    } = input;
    let id_const = match id {
        Some(id) => quote! { const ID: Option<i32> = Some(#id); },
        None => quote! {},
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let output = quote! {
        impl #impl_generics PacketRead for #ident #ty_generics #where_clause {
            #id_const
        }
    };
    output.into()
}
