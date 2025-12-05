use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::DeriveInput;
use syn::{parse_macro_input, token, FnArg, Ident, ItemFn, Pat, Type};

/// Input structure for the system! macro
///
/// Example:
/// ```ignore
/// system!(SystemName {
///     query fn update(a: View<A>, b: View<B>) {
///         // user code
///     }
/// });
/// ```
struct SystemInput {
    system_name: Ident,
    _brace: token::Brace,
    query_fn: ItemFn,
    none_types: Vec<Type>,
    all_types: Vec<Type>,
    changed_types: Vec<Type>,
}

impl Parse for SystemInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let system_name: Ident = input.parse()?;

        let content;
        let _brace = syn::braced!(content in input);

        // Skip "query" keyword
        let _query: Ident = content.parse()?;

        let query_fn: ItemFn = content.parse()?;

        let mut none_types = Vec::new();
        let mut all_types = Vec::new();
        let mut changed_types = Vec::new();
        while !content.is_empty() {
            let kw: Ident = content.parse()?;
            if kw == "None" {
                let _: token::Eq = content.parse()?;
                let inner;
                let _bracket = syn::bracketed!(inner in content);
                while !inner.is_empty() {
                    let ty: Type = inner.parse()?;
                    none_types.push(ty);
                    if inner.peek(syn::Token![,]) {
                        let _comma: syn::Token![,] = inner.parse()?;
                    }
                }
            } else if kw == "All" {
                let _: token::Eq = content.parse()?;
                let inner;
                let _bracket = syn::bracketed!(inner in content);
                while !inner.is_empty() {
                    let ty: Type = inner.parse()?;
                    all_types.push(ty);
                    if inner.peek(syn::Token![,]) {
                        let _comma: syn::Token![,] = inner.parse()?;
                    }
                }
            } else if kw == "Changed" {
                let _: token::Eq = content.parse()?;
                let inner;
                let _bracket = syn::bracketed!(inner in content);
                while !inner.is_empty() {
                    let ty: Type = inner.parse()?;
                    changed_types.push(ty);
                    if inner.peek(syn::Token![,]) {
                        let _comma: syn::Token![,] = inner.parse()?;
                    }
                }
            } else {
                return Err(syn::Error::new_spanned(
                    kw,
                    "Expected None=[...], All=[...], or Changed=[...]",
                )
                .into());
            }
            if content.peek(syn::Token![,]) {
                let _comma: syn::Token![,] = content.parse()?;
            }
        }

        Ok(SystemInput {
            system_name,
            _brace: _brace,
            query_fn,
            none_types,
            all_types,
            changed_types,
        })
    }
}

/// Extracts component types and parameter info from View<T> parameters
/// Handles both View<T>, ViewMut<T>, and &mut ViewMut<T> patterns
fn extract_view_params(query_fn: &ItemFn) -> Vec<(Ident, Type, bool)> {
    let mut params = Vec::new();

    for arg in &query_fn.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                // Check if it's a reference type (&mut ViewMut<T>)
                let inner_type = if let Type::Reference(type_ref) = &*pat_type.ty {
                    &*type_ref.elem
                } else {
                    &*pat_type.ty
                };

                if let Type::Path(type_path) = inner_type {
                    let last_segment = &type_path.path.segments.last().unwrap();
                    let type_name = &last_segment.ident;

                    let is_mut = type_name == "ViewMut";

                    if type_name == "View" || is_mut {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if let Some(syn::GenericArgument::Type(component_type)) =
                                args.args.first()
                            {
                                params.push((
                                    pat_ident.ident.clone(),
                                    component_type.clone(),
                                    is_mut,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    params
}

#[proc_macro]
pub fn system(input: TokenStream) -> TokenStream {
    let SystemInput {
        system_name,
        query_fn,
        none_types,
        all_types,
        changed_types,
        ..
    } = parse_macro_input!(input as SystemInput);

    let params = extract_view_params(&query_fn);

    if params.is_empty() {
        return syn::Error::new_spanned(
            &query_fn.sig,
            "Query function must have at least one View<T> or ViewMut<T> parameter",
        )
        .to_compile_error()
        .into();
    }

    let query_fn_name = &query_fn.sig.ident;
    let query_fn_body = &query_fn.block;
    let query_params = &query_fn.sig.inputs;

    // Build required types from params + All + Changed; negative-only from None
    let mut required_index: HashMap<String, usize> = HashMap::new();
    let mut required_types: Vec<(Type, bool)> = Vec::new(); // (Type, is_mut)
    for (_, ty, is_mut) in &params {
        let key = quote! { #ty }.to_string();
        if let Some(idx) = required_index.get_mut(&key) {
            if *is_mut {
                required_types[*idx].1 = true;
            }
        } else {
            required_index.insert(key, required_types.len());
            required_types.push((ty.clone(), *is_mut));
        }
    }
    for ty in &all_types {
        let key = quote! { #ty }.to_string();
        if !required_index.contains_key(&key) {
            required_index.insert(key.clone(), required_types.len());
            required_types.push((ty.clone(), false));
        }
    }
    for ty in &changed_types {
        let key = quote! { #ty }.to_string();
        if !required_index.contains_key(&key) {
            required_index.insert(key.clone(), required_types.len());
            required_types.push((ty.clone(), false));
        }
    }
    let required_count = required_types.len();

    let mut negative_types: Vec<Type> = Vec::new();
    for ty in &none_types {
        let key = quote! { #ty }.to_string();
        if !required_index.contains_key(&key) {
            negative_types.push(ty.clone());
        }
    }

    // Build unified storage fields
    let mut storage_fields: Vec<(Ident, Type, proc_macro2::TokenStream, bool)> = Vec::new();
    for (i, (ty, is_mut)) in required_types.iter().enumerate() {
        let field_name = Ident::new(&format!("storage_{}", i), system_name.span());
        let mutability = if *is_mut {
            quote! { mut }
        } else {
            quote! { const }
        };
        storage_fields.push((field_name, ty.clone(), mutability, *is_mut));
    }
    for (j, ty) in negative_types.iter().enumerate() {
        let i = required_count + j;
        let field_name = Ident::new(&format!("storage_{}", i), system_name.span());
        storage_fields.push((field_name, ty.clone(), quote! { const }, false));
    }

    // Map each param to its storage index
    let param_storage_indices: Vec<usize> = params
        .iter()
        .map(|(_, ty, _)| {
            let key = quote! { #ty }.to_string();
            *required_index
                .get(&key)
                .expect("param type must be in required_index")
        })
        .collect();
    let param_chunk_idents: Vec<Ident> = param_storage_indices
        .iter()
        .map(|idx| Ident::new(&format!("chunk_{}", idx), system_name.span()))
        .collect();

    let mut read_keys: HashMap<String, bool> = HashMap::new();
    let mut read_types: Vec<proc_macro2::TokenStream> = Vec::new();
    for (_, ty, is_mut) in &params {
        if !is_mut {
            let key = quote! { #ty }.to_string();
            if read_keys.insert(key.clone(), true).is_none() {
                read_types.push(quote! { std::any::TypeId::of::<#ty>() });
            }
        }
    }
    for ty in &all_types {
        let key = quote! { #ty }.to_string();
        if read_keys.insert(key.clone(), true).is_none() {
            read_types.push(quote! { std::any::TypeId::of::<#ty>() });
        }
    }
    for ty in &changed_types {
        let key = quote! { #ty }.to_string();
        if read_keys.insert(key.clone(), true).is_none() {
            read_types.push(quote! { std::any::TypeId::of::<#ty>() });
        }
    }

    let write_types: Vec<_> = params
        .iter()
        .filter(|(_, _, is_mut)| *is_mut)
        .map(|(_, ty, _)| quote! { std::any::TypeId::of::<#ty>() })
        .collect();

    let struct_fields = storage_fields
        .iter()
        .map(|(field_name, ty, mutability, _)| {
            quote! { pub #field_name: *#mutability decs::storage::Storage<#ty> }
        });
    let debug_struct_fields = quote! {};

    let new_storage_init = storage_fields.iter().map(|(field_name, ty, _, is_mut)| {
        if *is_mut {
            quote! {
                let #field_name = {
                    world.get_storage::<#ty>()
                };
            }
        } else {
            quote! {
                let #field_name = {
                    world.get_storage::<#ty>() as *const decs::storage::Storage<#ty>
                };
            }
        }
    });

    let new_field_init = storage_fields.iter().map(|(field_name, _, _, _)| {
        quote! { #field_name }
    });
    let new_debug_init = quote! {};

    // Generate mask intersection iteration
    let first_storage = &storage_fields[0].0;
    let mask_intersection = if required_count <= 1 {
        quote! { (*self.#first_storage).presence_mask }
    } else {
        let rest_indices: Vec<usize> = (1..required_count).collect();
        let rest_storages = rest_indices.iter().map(|i| {
            let name = &storage_fields[*i].0;
            quote! { & (*self.#name).presence_mask }
        });
        quote! { (*self.#first_storage).presence_mask #(#rest_storages)* }
    };
    let none_full_pages_or = if storage_fields.len() == required_count {
        quote! { 0u64 }
    } else {
        let ors = (required_count..storage_fields.len()).map(|i| {
            let name = &storage_fields[i].0;
            quote! { (*self.#name).fullness_mask }
        });
        quote! { 0u64 #(| #ors)* }
    };

    // Prepare parameter gathering using precomputed chunk references
    let param_gathering: Vec<_> = params
        .iter()
        .enumerate()
        .map(|(i, (param_name, _ty, is_mut))| {
            let chunk_var = &param_chunk_idents[i];
            if *is_mut {
                let storage_field = &storage_fields[param_storage_indices[i]].0;
                quote! {
                    let mut #param_name = decs::view::ViewMut::new(
                        #chunk_var,
                        chunk_item_idx as u32,
                        self.#storage_field,
                        storage_idx as u32,
                        page_idx as u32,
                        _frame.current_tick,
                    );
                }
            } else {
                quote! {
                    let #param_name = {
                        let data = #chunk_var.data[chunk_item_idx].assume_init_ref();
                        decs::view::View::new(data)
                    };
                }
            }
        })
        .collect();

    let call_args: Vec<_> = params
        .iter()
        .map(|(name, _, is_mut)| {
            if *is_mut {
                quote! { &mut #name }
            } else {
                quote! { #name }
            }
        })
        .collect();

    // Generate intersection operations across all storages at page and chunk levels,
    // and precompute page/chunk references for each storage
    let page_refs_init: Vec<_> = (0..required_count)
        .map(|i| {
            let (name, _ty, _mutability, is_mut) = &storage_fields[i];
            let page_var = Ident::new(&format!("page_{}", i), system_name.span());
            if *is_mut {
                quote! { let mut #page_var = &mut *(*self.#name).data[storage_idx]; }
            } else {
                quote! { let #page_var = &*(*self.#name).data[storage_idx]; }
            }
        })
        .collect();
    let page_mask_intersections: Vec<_> = (1..required_count)
        .map(|i| {
            let page_var = Ident::new(&format!("page_{}", i), system_name.span());
            quote! { page_mask &= #page_var.presence_mask; }
        })
        .collect();
    let mut changed_indices_set: std::collections::HashSet<usize> =
        std::collections::HashSet::new();
    for ty in &changed_types {
        let key = quote! { #ty }.to_string();
        if let Some(idx) = required_index.get(&key) {
            changed_indices_set.insert(*idx);
        }
    }
    let page_changed_intersections: Vec<_> = changed_indices_set
        .iter()
        .filter_map(|i| {
            if *i < required_count {
                let page_var = Ident::new(&format!("page_{}", i), system_name.span());
                Some(quote! { page_mask &= #page_var.changed_mask; })
            } else {
                None
            }
        })
        .collect();
    let chunk_refs_init: Vec<_> = (0..required_count).map(|i| {
        let is_mut = storage_fields[i].3;
        let page_var = Ident::new(&format!("page_{}", i), system_name.span());
        let chunk_var = Ident::new(&format!("chunk_{}", i), system_name.span());
        if is_mut {
            let storage_field = &storage_fields[i].0;
            quote! {
                {
                    let ct = _frame.current_tick;
                    if ((*self.#storage_field).rollback.tick() != ct) {
                        let new_current = if let Some(mut pooled) = (*self.#storage_field).rollback_pool.pop() {
                            pooled.reset_for_tick(ct);
                            pooled
                        } else {
                            Box::new(decs::rollback::RollbackStorage::with_tick(ct))
                        };
                        let old = std::mem::replace(&mut (*self.#storage_field).rollback, new_current);
                        let mut merged = std::mem::take(&mut (*self.#storage_field).prev);
                        merged.push_back(old);
                        (*self.#storage_field).prev = merged;
                        while (*self.#storage_field).prev.len() > 64 {
                            (*self.#storage_field).prev.pop_front();
                        }
                    }
                }
                let mut #chunk_var = &mut *#page_var.data[page_idx];
            }
        } else {
            quote! { let #chunk_var = &*#page_var.data[page_idx]; }
        }
    }).collect();
    let item_mask_intersections: Vec<_> = (1..required_count)
        .map(|i| {
            let chunk_var = Ident::new(&format!("chunk_{}", i), system_name.span());
            quote! { m &= #chunk_var.presence_mask; }
        })
        .collect();
    let item_changed_intersections: Vec<_> = changed_indices_set
        .iter()
        .filter_map(|i| {
            if *i < required_count {
                let chunk_var = Ident::new(&format!("chunk_{}", i), system_name.span());
                Some(quote! { m &= #chunk_var.changed_mask; })
            } else {
                None
            }
        })
        .collect();

    let none_chunk_full_or_inits: Vec<_> = if storage_fields.len() == required_count {
        Vec::new()
    } else {
        (required_count..storage_fields.len())
            .map(|i| {
                let name = &storage_fields[i].0;
                quote! {
                    {
                        let ns = &*self.#name;
                        let ns_page = &*(*ns).data[storage_idx];
                        none_chunk_full_or |= ns_page.fullness_mask;
                    }
                }
            })
            .collect()
    };

    let none_item_presence_or_inits: Vec<_> = if storage_fields.len() == required_count {
        Vec::new()
    } else {
        (required_count..storage_fields.len())
            .map(|i| {
                let name = &storage_fields[i].0;
                quote! {
                    {
                        let ns = &*self.#name;
                        let ns_page = &*(*ns).data[storage_idx];
                        let ns_chunk = &*ns_page.data[page_idx];
                        none_item_presence_or |= ns_chunk.presence_mask;
                    }
                }
            })
            .collect()
    };

    let propagate_changes: Vec<_> = storage_fields
        .iter()
        .enumerate()
        .filter(|(_, (_, _, _, is_mut))| *is_mut)
        .map(|(i, (name, _ty, _mutability, _))| {
            let page_var = Ident::new(&format!("page_{}", i), system_name.span());
            let chunk_var = Ident::new(&format!("chunk_{}", i), system_name.span());
            quote! {
                if #chunk_var.changed_mask != 0 {
                    #page_var.changed_mask |= 1u64 << page_idx;
                    (*self.#name).changed_mask |= 1u64 << storage_idx;
                }
            }
        })
        .collect();

    // Generate storage refresh sequences

    let expanded = quote! {
        pub struct #system_name {
            #(#struct_fields,)*
            #debug_struct_fields
        }

        unsafe impl Send for #system_name {}
        unsafe impl Sync for #system_name {}

        impl #system_name {
            pub fn new(world: &mut decs::world::World) -> Self {
                unsafe {
                    #(#new_storage_init)*

                    Self {
                        #(#new_field_init,)*
                        #new_debug_init
                    }
                }
            }

            fn #query_fn_name(#query_params) #query_fn_body
        }

        impl decs::system::System for #system_name {
            fn run(&self, _frame: &decs::frame::Frame) {
                unsafe {
                    let mut storage_mask = #mask_intersection & !#none_full_pages_or;
                    while storage_mask != 0 {
                        let storage_start = storage_mask.trailing_zeros() as usize;
                        let shifted = storage_mask >> storage_start;
                        let storage_run_len = shifted.trailing_ones() as usize;

                        for storage_idx in storage_start..storage_start + storage_run_len {
                            #(#page_refs_init)*
                            let mut page_mask = page_0.presence_mask;
                            #(#page_mask_intersections)*
                            #(#page_changed_intersections)*
                            let mut none_chunk_full_or: u64 = 0u64;
                            #(#none_chunk_full_or_inits)*
                            page_mask &= !none_chunk_full_or;


                            let mut page_mask_iter = page_mask;
                            while page_mask_iter != 0 {
                                let page_start = page_mask_iter.trailing_zeros() as usize;
                                let page_shifted = page_mask_iter >> page_start;
                                let page_run_len = page_shifted.trailing_ones() as usize;

                                for page_idx in page_start..page_start + page_run_len {
                                    #(#chunk_refs_init)*
                                    let mut item_mask = {
                                        let mut m = chunk_0.presence_mask;
                                        #(#item_mask_intersections)*
                                        #(#item_changed_intersections)*
                                        m
                                    };
                                    let mut none_item_presence_or: u64 = 0u64;
                                    #(#none_item_presence_or_inits)*
                                    item_mask &= !none_item_presence_or;


                                    let mut item_mask_iter = item_mask;
                                    while item_mask_iter != 0 {
                                        let item_start = item_mask_iter.trailing_zeros() as usize;
                                        let item_shifted = item_mask_iter >> item_start;
                                        let item_run_len = item_shifted.trailing_ones() as usize;

                                        for chunk_item_idx in item_start..item_start + item_run_len {
                                            #(#param_gathering)*
                                            #system_name::#query_fn_name(#(#call_args),*);
                                            #(#propagate_changes)*
                                        }
                                        item_mask_iter &= !((u64::MAX >> (64 - item_run_len)) << item_start);
                                    }

                                }
                                page_mask_iter &= !((u64::MAX >> (64 - page_run_len)) << page_start);
                            }

                        }
                        storage_mask &= !((u64::MAX >> (64 - storage_run_len)) << storage_start);
                    }

                }
            }

            fn reads(&self) -> &[std::any::TypeId] {
                static READS: &[std::any::TypeId] = &[#(#read_types),*];
                READS
            }

            fn writes(&self) -> &[std::any::TypeId] {
                static WRITES: &[std::any::TypeId] = &[#(#write_types),*];
                WRITES
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn debug_counts(&self) -> (usize, usize) { (0, 0) }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let id_mod = format_ident!("__decs_component_id_{}", name);
    let defaults_mod = format_ident!("__decs_component_defaults_{}", name);
    TokenStream::from(quote! {
        #[allow(non_snake_case)]
        mod #id_mod {
            pub(super) static mut ID: u32 = u32::MAX;
        }
        impl #impl_generics decs::component::Component for #name #ty_generics #where_clause {
            fn id() -> u32 {
                unsafe { self::#id_mod::ID }
            }
            fn initialize(id: u32) {
                unsafe {
                    if self::#id_mod::ID == u32::MAX {
                        self::#id_mod::ID = id;
                    }
                }
            }

            fn schedule_cleanup_system(world: &mut decs::world::World) {
                let sys = decs::system::ComponentCleanupSystem::<#name>::new(world);
                world.scheduler_mut().add_system(sys);
            }
        }

    })
}
struct SystemGroupInput {
    group_name: Ident,
    before_types: Vec<Type>,
    after_types: Vec<Type>,
    parent_type: Option<Type>,
}

impl Parse for SystemGroupInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let group_name: Ident = input.parse()?;
        let content;
        let _brace = syn::braced!(content in input);
        let mut before_types = Vec::new();
        let mut after_types = Vec::new();
        let mut parent_type: Option<Type> = None;
        while !content.is_empty() {
            let kw: Ident = content.parse()?;
            if kw == "Before" {
                let _: token::Eq = content.parse()?;
                let inner;
                let _bracket = syn::bracketed!(inner in content);
                while !inner.is_empty() {
                    let ty: Type = inner.parse()?;
                    before_types.push(ty);
                    if inner.peek(syn::Token![,]) {
                        let _comma: syn::Token![,] = inner.parse()?;
                    }
                }
            } else if kw == "After" {
                let _: token::Eq = content.parse()?;
                let inner;
                let _bracket = syn::bracketed!(inner in content);
                while !inner.is_empty() {
                    let ty: Type = inner.parse()?;
                    after_types.push(ty);
                    if inner.peek(syn::Token![,]) {
                        let _comma: syn::Token![,] = inner.parse()?;
                    }
                }
            } else if kw == "Parent" {
                let _: token::Eq = content.parse()?;
                let ty: Type = content.parse()?;
                parent_type = Some(ty);
            } else {
                return Err(syn::Error::new_spanned(
                    kw,
                    "Expected Before=[], After=[], or Parent=...",
                ));
            }
            if content.peek(syn::Token![,]) {
                let _comma: syn::Token![,] = content.parse()?;
            }
        }
        Ok(SystemGroupInput {
            group_name: group_name,
            before_types,
            after_types,
            parent_type,
        })
    }
}

#[proc_macro]
pub fn system_group(input: TokenStream) -> TokenStream {
    let SystemGroupInput {
        group_name,
        before_types,
        after_types,
        parent_type,
    } = parse_macro_input!(input as SystemGroupInput);
    let before_ids: Vec<_> = before_types
        .iter()
        .map(|ty| quote! { std::any::TypeId::of::<#ty>() })
        .collect();
    let after_ids: Vec<_> = after_types
        .iter()
        .map(|ty| quote! { std::any::TypeId::of::<#ty>() })
        .collect();
    let parent_static_ident = format_ident!("__decs_group_parent_{}", group_name);
    let parent_static = if let Some(ref ty) = parent_type {
        quote! { static #parent_static_ident: #ty = #ty; }
    } else {
        quote! {}
    };
    let parent_expr = if parent_type.is_some() {
        quote! { Some(&#parent_static_ident) }
    } else {
        quote! { None }
    };
    let self_static_ident = format_ident!("__decs_group_instance_{}", group_name);
    let expanded = quote! {
        pub struct #group_name;
        unsafe impl Send for #group_name {}
        unsafe impl Sync for #group_name {}
        static #self_static_ident: #group_name = #group_name;
        impl decs::system::SystemGroup for #group_name {
            fn instance() -> &'static dyn decs::system::SystemGroup where Self: Sized { &#self_static_ident }
            fn before(&self) -> &'static [std::any::TypeId] { static B: &[std::any::TypeId] = &[#(#before_ids),*]; B }
            fn after(&self) -> &'static [std::any::TypeId] { static A: &[std::any::TypeId] = &[#(#after_ids),*]; A }
            fn parent(&self) -> Option<&dyn decs::system::SystemGroup> { #parent_expr }
            fn as_any(&self) -> &dyn std::any::Any { self }
        }
        #parent_static
    };
    TokenStream::from(expanded)
}
