//! Standalone Rust type-contract tests.

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

    use argx::{
        ContractType as _, PrimitiveType, TYPE_CONTRACT_VERSION, TypeContractValue,
        TypeDefinitionKind, TypeVariantKind,
    };

    /// A documented reusable leaf.
    ///
    /// Later paragraphs are intentionally not part of the concise description.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    struct Identity {
        /// Stable identifier.
        id: u64,
    }

    /// One recursive tree node.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    struct Node {
        /// Child nodes.
        children: Vec<Self>,
        /// Optional shared identity.
        identity: Option<Box<Identity>>,
    }

    /// Left side of a mutually recursive pair.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    struct Left {
        right: Option<Box<Right>>,
    }

    /// Right side of a mutually recursive pair.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    struct Right {
        left: Option<Box<Left>>,
    }

    /// Representative enum payload shapes.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    enum Event {
        /// No payload.
        Started,
        /// One tuple payload.
        Count(u32),
        /// Named payload.
        Renamed {
            /// New display name.
            name: String,
        },
    }

    /// Generic declaration used in multiple monomorphizations.
    #[expect(dead_code, reason = "shape is exercised through the generated contract")]
    #[derive(argx::Contract)]
    struct Wrapper<T> {
        value: T,
    }

    /// Const-generic declaration.
    #[derive(argx::Contract)]
    struct Fixed<const N: usize>([u8; N]);

    #[test]
    fn primitives_and_standard_containers_have_semantic_shapes() {
        assert_eq!(
            bool::type_contract().root,
            TypeContractValue::Primitive { primitive: PrimitiveType::Bool },
        );
        assert_eq!(String::type_contract().root, TypeContractValue::String);
        assert_eq!(std::ffi::OsString::type_contract().root, TypeContractValue::OsString);
        assert_eq!(std::path::PathBuf::type_contract().root, TypeContractValue::Path);
        let infallible = std::convert::Infallible::type_contract();
        assert_eq!(
            infallible.root,
            TypeContractValue::Reference { definition: "type-0".to_owned() },
        );
        assert!(matches!(
            &infallible.definitions[0].kind,
            TypeDefinitionKind::Enum { variants } if variants.is_empty()
        ));
        assert_eq!(
            Option::<u32>::type_contract().root,
            TypeContractValue::Optional {
                value: Box::new(TypeContractValue::Primitive { primitive: PrimitiveType::U32 }),
            },
        );
        assert!(matches!(Vec::<u8>::type_contract().root, TypeContractValue::Sequence { .. }));
        assert!(matches!(BTreeSet::<u8>::type_contract().root, TypeContractValue::Set { .. }));
        assert!(matches!(HashSet::<u8>::type_contract().root, TypeContractValue::Set { .. }));
        assert!(matches!(
            BTreeMap::<String, u8>::type_contract().root,
            TypeContractValue::Map { .. }
        ));
        assert!(matches!(
            HashMap::<String, u8>::type_contract().root,
            TypeContractValue::Map { .. }
        ));
        assert!(matches!(
            <[u8; 4]>::type_contract().root,
            TypeContractValue::Array { length: 4, .. }
        ));
        assert!(matches!(<(String, bool)>::type_contract().root, TypeContractValue::Tuple { .. }));
    }

    #[test]
    fn derived_structs_use_named_definitions_and_terminate_recursion() {
        let contract = Node::type_contract();
        assert_eq!(contract.version, TYPE_CONTRACT_VERSION);
        assert_eq!(contract.root, TypeContractValue::Reference { definition: "type-0".to_owned() },);
        assert_eq!(contract.definitions.len(), 2);

        let node = &contract.definitions[0];
        assert_eq!(node.id, "type-0");
        assert_eq!(node.name, "Node");
        assert_eq!(node.description.as_deref(), Some("One recursive tree node."));
        let TypeDefinitionKind::Struct { fields } = &node.kind else {
            panic!("Node must resolve to a struct definition");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name.as_deref(), Some("children"));
        assert_eq!(fields[0].description.as_deref(), Some("Child nodes."));
        assert_eq!(
            fields[0].value,
            TypeContractValue::Sequence {
                value: Box::new(TypeContractValue::Reference { definition: "type-0".to_owned() }),
            },
        );
        assert_eq!(fields[1].name.as_deref(), Some("identity"));
        assert_eq!(
            fields[1].value,
            TypeContractValue::Optional {
                value: Box::new(TypeContractValue::Reference { definition: "type-1".to_owned() }),
            },
        );

        let identity = &contract.definitions[1];
        assert_eq!(identity.name, "Identity");
        assert_eq!(identity.description.as_deref(), Some("A documented reusable leaf."));
    }

    #[test]
    fn mutually_recursive_definitions_are_reserved_before_descent() {
        let contract = Left::type_contract();
        assert_eq!(contract.definitions.len(), 2);
        assert_eq!(contract.definitions[0].name, "Left");
        assert_eq!(contract.definitions[1].name, "Right");
    }

    #[test]
    fn enum_contracts_preserve_variant_and_field_shapes() {
        let contract = Event::type_contract();
        let TypeDefinitionKind::Enum { variants } = &contract.definitions[0].kind else {
            panic!("Event must resolve to an enum definition");
        };
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name, "Started");
        assert_eq!(variants[0].description.as_deref(), Some("No payload."));
        assert_eq!(variants[0].kind, TypeVariantKind::Unit);
        assert!(matches!(&variants[1].kind, TypeVariantKind::Tuple { .. }));
        let TypeVariantKind::Struct { fields } = &variants[2].kind else {
            panic!("Renamed must preserve its named payload");
        };
        assert_eq!(fields[0].name.as_deref(), Some("name"));
        assert_eq!(fields[0].description.as_deref(), Some("New display name."));
    }

    #[test]
    fn repeated_named_types_are_deduplicated_but_monomorphizations_are_distinct() {
        #[expect(dead_code, reason = "shape is exercised through the generated contract")]
        #[derive(argx::Contract)]
        struct Root {
            first: Identity,
            second: Identity,
            number: Wrapper<u32>,
            text: Wrapper<String>,
        }

        let contract = Root::type_contract();
        assert_eq!(contract.definitions.iter().filter(|item| item.name == "Identity").count(), 1);
        assert_eq!(contract.definitions.iter().filter(|item| item.name == "Wrapper").count(), 2);
    }

    #[test]
    fn semantically_equivalent_container_arguments_share_generic_definitions() {
        #[expect(dead_code, reason = "shape is exercised through the generated contract")]
        #[derive(argx::Contract)]
        struct Root {
            vector: Wrapper<Vec<u8>>,
            queue: Wrapper<VecDeque<u8>>,
        }

        let contract = Root::type_contract();
        assert_eq!(contract.definitions.iter().filter(|item| item.name == "Wrapper").count(), 1);
    }

    #[test]
    fn const_generics_produce_distinct_named_definitions() {
        #[expect(dead_code, reason = "shape is exercised through the generated contract")]
        #[derive(argx::Contract)]
        struct Root {
            four: Fixed<4>,
            eight: Fixed<8>,
        }

        let contract = Root::type_contract();
        let fixed =
            contract.definitions.iter().filter(|item| item.name == "Fixed").collect::<Vec<_>>();
        assert_eq!(fixed.len(), 2);
    }

    #[test]
    fn ownership_wrappers_do_not_change_semantic_type_shape() {
        assert_eq!(<&'static str>::type_contract().root, String::type_contract().root);
        assert_eq!(Box::<String>::type_contract().root, String::type_contract().root);
        assert_eq!(std::rc::Rc::<String>::type_contract().root, String::type_contract().root);
        assert_eq!(std::sync::Arc::<String>::type_contract().root, String::type_contract().root);
    }

    #[test]
    fn json_wire_shape_is_versioned_and_stable() {
        #[expect(dead_code, reason = "shape is exercised through the generated contract")]
        #[derive(argx::Contract)]
        struct Payload {
            name: String,
            count: Option<u32>,
        }

        let json = Payload::type_contract().to_json_pretty().expect("type contract must serialize");
        snapbox::Assert::new().action_env("SNAPSHOTS").eq(
            json,
            snapbox::str![[r#"
{
  "version": 1,
  "root": {
    "kind": "reference",
    "definition": "type-0"
  },
  "definitions": [
    {
      "id": "type-0",
      "name": "Payload",
      "kind": "struct",
      "fields": [
        {
          "name": "name",
          "value": {
            "kind": "string"
          }
        },
        {
          "name": "count",
          "value": {
            "kind": "optional",
            "value": {
              "kind": "primitive",
              "primitive": "u32"
            }
          }
        }
      ]
    }
  ]
}
"#]],
        );
    }

    #[test]
    fn serialized_contract_uses_document_local_references() {
        let json = Node::type_contract().to_json().expect("type contract must serialize");
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"definition\":\"type-0\""));
        assert!(!json.contains("type_contract::tests::Node"));
    }
}
