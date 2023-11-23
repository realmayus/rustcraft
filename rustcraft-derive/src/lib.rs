use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields};

/// This derives the `SizedProt` trait for structs
#[proc_macro_derive(SizedProt)]
pub fn derive_sized_prot(input: TokenStream) -> TokenStream {
    // Parse it as a proc macro
    let input = parse_macro_input!(input as DeriveInput);

    if let syn::Data::Struct(ref data) = input.data {
        if let Fields::Named(ref fields) = data.fields {
            let field_vals = fields.named.iter().enumerate().map(|(i, field)| {
                let name = &field.ident;
                quote!(self.#name.prot_size())
            });
            let name = input.ident;

            return TokenStream::from(quote!(
                impl crate::protocol_types::traits::SizedProt for #name {
                    fn prot_size(&self) -> usize {
                         0 #(+ #field_vals)*
                    }
                }
            ));
        }
    }

    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Only structs with named fields can derive `SizedProt`",
        )
        .to_compile_error(),
    )
}

#[proc_macro_derive(WriteProtPacket)]
pub fn derive_write_prot_packet(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if let syn::Data::Enum(ref data) = input.data {
        let name = input.ident;
        let mut match_arms = Vec::new();
        for variant in &data.variants {
            let variant_name = &variant.ident;
            let ident = quote!(f);
            match_arms.push(quote!(#name::#variant_name(#ident) => #ident.write(stream, connection).await?,));
        }
        return TokenStream::from(quote!(
            #[async_trait]
            impl crate::protocol_types::traits::WriteProtPacket for #name {
                async fn write(
                        &self,
                        stream: &mut (impl AsyncWrite + Unpin + Send),
                        connection: &mut Connection,
                    ) -> Result<(), String> {
                    match self {
                        #(#match_arms)*
                    }
                    Ok(())
                }
            }
        ));
    }
    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Only structs with named fields can derive `WriteProt`",
        )
            .to_compile_error(),
    )
}

/// This derives the `WriteProt` trait for structs
#[proc_macro_derive(WriteProt)]
pub fn derive_write_prot(input: TokenStream) -> TokenStream {
    // Parse it as a proc macro
    let input = parse_macro_input!(input as DeriveInput);

    if let syn::Data::Struct(ref data) = input.data {
        if let Fields::Named(ref fields) = data.fields {
            let field_vals = fields.named.iter().enumerate().map(|(i, field)| {
                let name = &field.ident;
                quote!(self.#name.write(stream).await?;)
            });
            let name = input.ident;
            return TokenStream::from(quote!(
                #[async_trait]
                impl crate::protocol_types::traits::WriteProt for #name {
                    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
                        #(#field_vals)*
                        Ok(())
                    }
                }
            ));
        }
    }

    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Only structs with named fields can derive `WriteProt`",
        )
        .to_compile_error(),
    )
}

/// This derives the `ReadProt` trait for structs
#[proc_macro_derive(ReadProt)]
pub fn derive_read_prot(input: TokenStream) -> TokenStream {
    // Parse it as a proc macro
    let input = parse_macro_input!(input as DeriveInput);

    if let syn::Data::Struct(ref data) = input.data {
        if let Fields::Named(ref fields) = data.fields {
            let field_vals = fields.named.iter().enumerate().map(|(i, field)| {
                let name = &field.ident;
                let ty = &field.ty;
                quote!(#name: <#ty>::read(stream).await?,)
            });
            let name = input.ident;
            return TokenStream::from(quote!(
                #[async_trait]
                impl crate::protocol_types::traits::ReadProt for #name {
                    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String> where Self: Sized {
                        Ok( #name {
                                #(#field_vals)*
                        })
                    }
                }
            ));
        }
    }

    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Only structs with named fields can derive `ReadProt`",
        )
        .to_compile_error(),
    )
}
