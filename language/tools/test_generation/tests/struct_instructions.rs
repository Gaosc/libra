extern crate test_generation;
use std::collections::HashMap;
use test_generation::abstract_state::{AbstractState, AbstractValue};
use vm::{
    access::ModuleAccess,
    file_format::{
        empty_module, Bytecode, CompiledModule, CompiledModuleMut, FieldDefinition,
        FieldDefinitionIndex, Kind, LocalsSignatureIndex, MemberCount, ModuleHandleIndex,
        SignatureToken, StringPoolIndex, StructDefinition, StructDefinitionIndex,
        StructFieldInformation, StructHandle, StructHandleIndex, TableIndex, TypeSignature,
        TypeSignatureIndex,
    },
    views::{SignatureTokenView, StructDefinitionView, ViewInternals},
};

mod common;

fn generate_module_with_struct(resource: bool) -> CompiledModuleMut {
    let mut module: CompiledModuleMut = empty_module();
    module.type_signatures = vec![
        SignatureToken::Bool,
        SignatureToken::U64,
        SignatureToken::String,
        SignatureToken::ByteArray,
        SignatureToken::Address,
    ]
    .into_iter()
    .map(TypeSignature)
    .collect();

    let struct_index = 0;
    let num_fields = 5;
    let offset = module.string_pool.len() as TableIndex;
    module.string_pool.push("struct0".to_string());

    let field_information = StructFieldInformation::Declared {
        field_count: num_fields as MemberCount,
        fields: FieldDefinitionIndex::new(module.field_defs.len() as TableIndex),
    };
    let struct_def = StructDefinition {
        struct_handle: StructHandleIndex(struct_index),
        field_information,
    };
    module.struct_defs.push(struct_def);

    for i in 0..num_fields {
        module.string_pool.push(format!("string{}", i));
        let struct_handle_idx = StructHandleIndex::new(struct_index);
        let typ_idx = TypeSignatureIndex::new(0);
        let str_pool_idx = StringPoolIndex::new(i + 1 as TableIndex);
        let field_def = FieldDefinition {
            struct_: struct_handle_idx,
            name: str_pool_idx,
            signature: typ_idx,
        };
        module.field_defs.push(field_def);
    }
    module.struct_handles = vec![StructHandle {
        module: ModuleHandleIndex::new(0),
        name: StringPoolIndex::new((struct_index + offset) as TableIndex),
        is_nominal_resource: resource,
        type_formals: vec![],
    }];
    module
}

fn create_struct_value(module: &CompiledModule) -> AbstractValue {
    let struct_def = module.struct_def_at(StructDefinitionIndex::new(0));
    let struct_def_view = StructDefinitionView::new(module, struct_def);
    let tokens: Vec<SignatureToken> = struct_def_view
        .fields()
        .into_iter()
        .flatten()
        .map(|field| field.type_signature().token().as_inner().clone())
        .collect();
    let struct_kind = match struct_def_view.is_nominal_resource() {
        true => Kind::Resource,
        false => tokens
            .iter()
            .map(|token| SignatureTokenView::new(module, token).kind(&[]))
            .fold(Kind::Unrestricted, |acc_kind, next_kind| {
                match (acc_kind, next_kind) {
                    (Kind::All, _) | (_, Kind::All) => Kind::All,
                    (Kind::Resource, _) | (_, Kind::Resource) => Kind::Resource,
                    (Kind::Unrestricted, Kind::Unrestricted) => Kind::Unrestricted,
                }
            }),
    };
    AbstractValue::new_struct(
        SignatureToken::Struct(struct_def.struct_handle, tokens.clone()),
        struct_kind,
    )
}

#[test]
#[should_panic]
fn bytecode_pack_signature_not_satisfied() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let state1 = AbstractState::from_locals(module, HashMap::new());
    common::run_instruction(
        Bytecode::Pack(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
fn bytecode_pack() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    let struct_value1 = create_struct_value(&state1.module);
    if let SignatureToken::Struct(_, tokens) = struct_value1.clone().token {
        for token in tokens {
            let abstract_value = AbstractValue {
                token: token.clone(),
                kind: SignatureTokenView::new(&state1.module, &token).kind(&[]),
            };
            state1.stack_push(abstract_value);
        }
    }
    let state2 = common::run_instruction(
        Bytecode::Pack(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
    let struct_value2 = state2.stack_peek(0).expect("struct not added to stack");
    assert_eq!(
        struct_value1, struct_value2,
        "stack type postcondition not met"
    );
}

#[test]
#[should_panic]
fn bytecode_unpack_signature_not_satisfied() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let state1 = AbstractState::from_locals(module, HashMap::new());
    common::run_instruction(
        Bytecode::Unpack(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
fn bytecode_unpack() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    let struct_value = create_struct_value(&state1.module);
    state1.stack_push(struct_value.clone());
    let state2 = common::run_instruction(
        Bytecode::Unpack(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
    if let SignatureToken::Struct(_, tokens) = struct_value.token {
        assert_eq!(
            state2.stack_len(),
            tokens.len(),
            "stack type postcondition not met"
        );
    } else {
        panic!("Created struct is malformed");
    }
}

#[test]
fn bytecode_exists() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    state1.stack_push(AbstractValue::new_primitive(SignatureToken::Address));
    let state2 = common::run_instruction(
        Bytecode::Exists(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
    assert_eq!(
        state2.stack_peek(0),
        Some(AbstractValue::new_primitive(SignatureToken::Bool)),
        "stack type postcondition not met"
    );
}

#[test]
#[should_panic]
fn bytecode_exists_struct_is_not_resource() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    state1.stack_push(AbstractValue::new_primitive(SignatureToken::Address));
    common::run_instruction(
        Bytecode::Exists(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
#[should_panic]
fn bytecode_exists_no_address_on_stack() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let state1 = AbstractState::from_locals(module, HashMap::new());
    common::run_instruction(
        Bytecode::Exists(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
fn bytecode_movefrom() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    let state1_copy = state1.clone();
    let struct_def = state1_copy
        .module
        .struct_def_at(StructDefinitionIndex::new(0));
    state1.stack_push(AbstractValue::new_primitive(SignatureToken::Address));
    let state2 = common::run_instruction(
        Bytecode::MoveFrom(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
    let struct_value = state2.stack_peek(0).expect("struct not added to stack");
    assert!(
        match struct_value.token {
            SignatureToken::Struct(struct_handle, _) => struct_handle == struct_def.struct_handle,
            _ => false,
        },
        "stack type postcondition not met"
    );
}

#[test]
#[should_panic]
fn bytecode_movefrom_struct_is_not_resource() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    state1.stack_push(AbstractValue::new_primitive(SignatureToken::Address));
    common::run_instruction(
        Bytecode::MoveFrom(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
#[should_panic]
fn bytecode_movefrom_no_address_on_stack() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let state1 = AbstractState::from_locals(module, HashMap::new());
    common::run_instruction(
        Bytecode::MoveFrom(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
fn bytecode_movetosender() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    state1.stack_push(create_struct_value(&state1.module));
    let state2 = common::run_instruction(
        Bytecode::MoveToSender(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
    assert_eq!(state2.stack_len(), 0, "stack type postcondition not met");
}

#[test]
#[should_panic]
fn bytecode_movetosender_struct_is_not_resource() {
    let module: CompiledModuleMut = generate_module_with_struct(false);
    let mut state1 = AbstractState::from_locals(module, HashMap::new());
    state1.stack_push(create_struct_value(&state1.module));
    common::run_instruction(
        Bytecode::MoveToSender(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}

#[test]
#[should_panic]
fn bytecode_movetosender_no_struct_on_stack() {
    let module: CompiledModuleMut = generate_module_with_struct(true);
    let state1 = AbstractState::from_locals(module, HashMap::new());
    common::run_instruction(
        Bytecode::MoveToSender(StructDefinitionIndex::new(0), LocalsSignatureIndex::new(0)),
        state1,
    );
}
