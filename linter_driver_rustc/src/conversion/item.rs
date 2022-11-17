use std::cell::RefCell;

use linter_api::ast::{
    generic::{
        GenericParamKind, GenericParams, LifetimeClause, LifetimeParam, TraitBound, TyClause, TyParam, TyParamBound,
        WhereClauseKind,
    },
    item::{
        AssocItemKind, CommonItemData, ConstItem, ExternCrateItem, FnItem, ImplItem, ItemKind, ModItem, StaticItem,
        TyAliasItem, UseItem, UseKind,
    },
    ty::TyKind,
    CommonCallableData, ItemId, Parameter,
};
use rustc_hash::FxHashMap;
use rustc_hir as hir;

use crate::context::RustcContext;

use super::{
    generic::{to_api_lifetime, to_api_trait_ref},
    to_api_abi, to_api_body_id, to_api_generic_id, to_api_item_id_from_def_id, to_api_mutability, to_api_path,
    to_api_span_id, to_api_symbol_id,
    ty::to_api_syn_ty,
};

pub struct ItemConverter<'ast, 'tcx> {
    cx: &'ast RustcContext<'ast, 'tcx>,
    items: RefCell<FxHashMap<ItemId, ItemKind<'ast>>>,
}

impl<'ast, 'tcx> ItemConverter<'ast, 'tcx> {
    pub fn new(cx: &'ast RustcContext<'ast, 'tcx>) -> Self {
        Self {
            cx,
            items: RefCell::default(),
        }
    }

    pub fn conv_item(&self, rustc_item: &hir::Item<'tcx>) -> Option<ItemKind<'ast>> {
        let id = to_api_item_id_from_def_id(self.cx, rustc_item.owner_id.to_def_id());
        if let Some(item) = self.items.borrow().get(&id) {
            return Some(*item);
        }

        let name = to_api_symbol_id(rustc_item.ident.name);
        let data = CommonItemData::new(id, name);
        let item = match &rustc_item.kind {
            hir::ItemKind::ExternCrate(original_name) => ItemKind::ExternCrate(
                self.alloc(|| ExternCrateItem::new(data, original_name.map_or(name, to_api_symbol_id))),
            ),
            hir::ItemKind::Use(path, use_kind) => {
                let use_kind = match use_kind {
                    hir::UseKind::Single => UseKind::Single,
                    hir::UseKind::Glob => UseKind::Glob,
                    hir::UseKind::ListStem => return None,
                };
                ItemKind::Use(self.alloc(|| UseItem::new(data, to_api_path(self.cx, path), use_kind)))
            },
            hir::ItemKind::Static(rustc_ty, rustc_mut, rustc_body_id) => ItemKind::Static(self.alloc(|| {
                StaticItem::new(
                    data,
                    to_api_mutability(self.cx, *rustc_mut),
                    to_api_body_id(self.cx, *rustc_body_id),
                    self.conv_ty(rustc_ty),
                )
            })),
            hir::ItemKind::Const(rustc_ty, rustc_body_id) => ItemKind::Const(self.alloc(|| {
                ConstItem::new(
                    data,
                    self.conv_ty(rustc_ty),
                    Some(to_api_body_id(self.cx, *rustc_body_id)),
                )
            })),
            hir::ItemKind::Fn(fn_sig, generics, body_id) => ItemKind::Fn(self.alloc(|| {
                FnItem::new(
                    data,
                    self.conv_generic(generics),
                    self.conv_fn_sig(fn_sig, false),
                    Some(to_api_body_id(self.cx, *body_id)),
                )
            })),
            hir::ItemKind::Mod(rustc_mod) => {
                ItemKind::Mod(self.alloc(|| ModItem::new(data, self.conv_items(rustc_mod.item_ids))))
            },
            hir::ItemKind::ForeignMod { .. } => todo!(),
            hir::ItemKind::Macro(_, _) | hir::ItemKind::GlobalAsm(_) => return None,
            hir::ItemKind::TyAlias(rustc_ty, rustc_generics) => ItemKind::TyAlias(
                self.alloc(|| TyAliasItem::new(data, self.conv_generic(rustc_generics), Some(self.conv_ty(rustc_ty)))),
            ),
            hir::ItemKind::OpaqueTy(_) => todo!(),
            hir::ItemKind::Enum(_, _) => todo!(),
            hir::ItemKind::Struct(_, _) => todo!(),
            hir::ItemKind::Union(_, _) => todo!(),
            hir::ItemKind::Trait(_, _, _, _, _) => todo!(),
            hir::ItemKind::TraitAlias(_, _) => todo!(),
            hir::ItemKind::Impl(imp) => ItemKind::Impl(self.alloc(|| {
                ImplItem::new(
                    data,
                    matches!(imp.unsafety, hir::Unsafety::Unsafe),
                    matches!(imp.polarity, rustc_ast::ImplPolarity::Positive),
                    imp.of_trait
                        .as_ref()
                        .map(|trait_ref| to_api_trait_ref(self.cx, trait_ref)),
                    self.conv_generic(imp.generics),
                    to_api_syn_ty(self.cx, imp.self_ty),
                    self.conv_assoc_items(imp.items),
                )
            })),
        };

        self.items.borrow_mut().insert(id, item);
        Some(item)
    }

    pub fn conv_items(&self, item: &[hir::ItemId]) -> &'ast [ItemKind<'ast>] {
        #[expect(
            clippy::needless_collect,
            reason = "collect is required to know the size of the allocation"
        )]
        let items: Vec<ItemKind<'_>> = item
            .iter()
            .map(|rid| self.cx.rustc_cx.hir().item(*rid))
            .filter_map(|rustc_item| self.conv_item(rustc_item))
            .collect();
        self.cx.storage.alloc_slice_iter(items.into_iter())
    }

    fn conv_assoc_item(&self, rustc_item: &hir::ImplItemRef) -> AssocItemKind<'ast> {
        let id = to_api_item_id_from_def_id(self.cx, rustc_item.id.owner_id.to_def_id());
        if let Some(item) = self.items.borrow().get(&id) {
            return item.try_into().unwrap();
        }

        let impl_item = self.cx.rustc_cx.hir().impl_item(rustc_item.id);
        let name = to_api_symbol_id(rustc_item.ident.name);
        let data = CommonItemData::new(id, name);

        let item = match &impl_item.kind {
            hir::ImplItemKind::Const(ty, body_id) => AssocItemKind::Const(
                self.alloc(|| ConstItem::new(data, self.conv_ty(ty), Some(to_api_body_id(self.cx, *body_id)))),
            ),
            hir::ImplItemKind::Fn(fn_sig, body_id) => AssocItemKind::Fn(self.alloc(|| {
                FnItem::new(
                    data,
                    self.conv_generic(impl_item.generics),
                    self.conv_fn_sig(fn_sig, false),
                    Some(to_api_body_id(self.cx, *body_id)),
                )
            })),
            hir::ImplItemKind::Type(ty) => AssocItemKind::TyAlias(
                self.alloc(|| TyAliasItem::new(data, self.conv_generic(impl_item.generics), Some(self.conv_ty(ty)))),
            ),
        };

        self.items.borrow_mut().insert(id, item.as_item());
        item
    }

    pub fn conv_assoc_items(&self, items: &[hir::ImplItemRef]) -> &'ast [AssocItemKind<'ast>] {
        self.cx
            .storage
            .alloc_slice_iter(items.iter().map(|item| self.conv_assoc_item(item)))
    }

    fn conv_ty(&self, rustc_ty: &'tcx hir::Ty<'tcx>) -> TyKind<'ast> {
        to_api_syn_ty(self.cx, rustc_ty)
    }

    fn conv_generic_params(&self, params: &[hir::GenericParam<'ast>]) -> &'ast [GenericParamKind<'ast>] {
        if params.is_empty() {
            return &[];
        }

        let params: Vec<_> = params
            .iter()
            .filter_map(|rustc_param| {
                let name = match rustc_param.name {
                    hir::ParamName::Plain(ident) => to_api_symbol_id(ident.name),
                    _ => return None,
                };
                let id = to_api_generic_id(self.cx, rustc_param.hir_id.expect_owner().to_def_id());
                let span = to_api_span_id(self.cx, rustc_param.span);
                match rustc_param.kind {
                    hir::GenericParamKind::Lifetime {
                        kind: hir::LifetimeParamKind::Explicit,
                    } => Some(GenericParamKind::Lifetime(
                        self.alloc(|| LifetimeParam::new(id, name, &[], Some(span))),
                    )),
                    hir::GenericParamKind::Type { synthetic: false, .. } => Some(GenericParamKind::Ty(
                        self.alloc(|| TyParam::new(Some(span), name, id, &[])),
                    )),
                    _ => None,
                }
            })
            .collect();

        self.cx.storage.alloc_slice_iter(params.into_iter())
    }

    fn conv_generic_bounds(&self, bounds: hir::GenericBounds<'tcx>) -> &'ast [TyParamBound<'ast>] {
        if bounds.is_empty() {
            return &[];
        }

        let bounds: Vec<_> = bounds
            .iter()
            .filter_map(|bound| match bound {
                hir::GenericBound::Trait(trait_ref, modifier) => Some(TyParamBound::TraitBound(self.alloc(|| {
                    TraitBound::new(
                        !matches!(modifier, hir::TraitBoundModifier::None),
                        to_api_trait_ref(self.cx, &trait_ref.trait_ref),
                        to_api_span_id(self.cx, bound.span()),
                    )
                }))),
                hir::GenericBound::LangItemTrait(_, _, _, _) => todo!(),
                hir::GenericBound::Outlives(rust_lt) => {
                    to_api_lifetime(self.cx, rust_lt).map(|api_lt| TyParamBound::Lifetime(self.alloc(|| api_lt)))
                },
            })
            .collect();

        self.cx.storage.alloc_slice_iter(bounds.into_iter())
    }

    fn conv_generic(&self, rustc_generics: &hir::Generics<'tcx>) -> GenericParams<'ast> {
        // FIXME: Update implementation to store the parameter bounds in the parameters
        let clauses: Vec<_> = rustc_generics
            .predicates
            .iter()
            .filter_map(|predicate| {
                match predicate {
                    hir::WherePredicate::BoundPredicate(ty_bound) => {
                        // FIXME Add span to API clause:
                        // let span = to_api_span_id(self.cx, ty_bound.span);
                        let params = GenericParams::new(self.conv_generic_params(ty_bound.bound_generic_params), &[]);
                        let ty = self.conv_ty(ty_bound.bounded_ty);
                        Some(WhereClauseKind::Ty(self.alloc(|| {
                            TyClause::new(Some(params), ty, self.conv_generic_bounds(predicate.bounds()))
                        })))
                    },
                    hir::WherePredicate::RegionPredicate(lifetime_bound) => {
                        to_api_lifetime(self.cx, lifetime_bound.lifetime).map(|lifetime| {
                            WhereClauseKind::Lifetime(self.alloc(|| {
                                let bounds: Vec<_> = lifetime_bound
                                    .bounds
                                    .iter()
                                    .filter_map(|bound| match bound {
                                        hir::GenericBound::Outlives(lifetime) => to_api_lifetime(self.cx, lifetime),
                                        _ => unreachable!("lifetimes can only be bound by lifetimes"),
                                    })
                                    .collect();
                                let bounds = if bounds.is_empty() {
                                    self.cx.storage.alloc_slice_iter(bounds.into_iter())
                                } else {
                                    &[]
                                };
                                LifetimeClause::new(lifetime, bounds)
                            }))
                        })
                    },
                    hir::WherePredicate::EqPredicate(_) => {
                        unreachable!("the documentation states, that this is unsupported")
                    },
                }
            })
            .collect();
        let clauses = self.cx.storage.alloc_slice_iter(clauses.into_iter());

        GenericParams::new(self.conv_generic_params(rustc_generics.params), clauses)
    }

    fn conv_fn_sig(&self, fn_sig: &hir::FnSig<'tcx>, is_extern: bool) -> CommonCallableData<'ast> {
        let params = self
            .cx
            .storage
            .alloc_slice_iter(fn_sig.decl.inputs.iter().map(|input_ty| {
                Parameter::new(
                    // FIXME: This should actually be a pattern, that can be
                    // retrieved from the body. For now this is kind of blocked
                    // by #50
                    None,
                    Some(to_api_syn_ty(self.cx, input_ty)),
                    Some(to_api_span_id(self.cx, input_ty.span)),
                )
            }));
        let header = fn_sig.header;
        let return_ty = if let hir::FnRetTy::Return(rust_ty) = fn_sig.decl.output {
            Some(to_api_syn_ty(self.cx, rust_ty))
        } else {
            None
        };
        CommonCallableData::new(
            header.is_const(),
            header.is_async(),
            header.is_unsafe(),
            is_extern,
            to_api_abi(self.cx, header.abi),
            fn_sig.decl.implicit_self.has_implicit_self(),
            params,
            return_ty,
        )
    }

    #[must_use]
    fn alloc<F, T>(&self, f: F) -> &'ast T
    where
        F: FnOnce() -> T,
    {
        self.cx.storage.alloc(f)
    }
}
