use bytes::Bytes;
use itertools::{self, Itertools};
use protobuf::descriptor::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    OneofDescriptorProto, ServiceDescriptorProto,
};
use protobuf::plugin::{code_generator_response, CodeGeneratorRequest, CodeGeneratorResponse};
use protobuf::Message;
use protobuf::{CodedInputStream, SpecialFields};
use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let iter = Bytes::from_iter(stdin.bytes().map(|x| x.unwrap_or_default()));
    while let Ok(request) = CodeGeneratorRequest::parse_from_tokio_bytes(&iter) {
        let code_generator = CodeGenerator::new(&request);
        let result = code_generator.generate();
        let bytes = result.write_to_bytes().unwrap();
        stdout.write(&bytes).unwrap();
    }
}

#[derive(Debug)]
enum CodeGeneratorException {}

#[derive(Debug, Default)]
struct CodeGenerator<'a> {
    protoc_version: String,
    request: &'a CodeGeneratorRequest,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(request: &'a CodeGeneratorRequest) -> Self {
        let protoc_version = if let Some(compiler_version) = &request.compiler_version.0 {
            format!(
                "{:?}{:?}{:?}",
                compiler_version.major(),
                compiler_version.minor(),
                compiler_version.patch()
            )
        } else {
            "".to_string()
        };

        struct TypeCollector<'s> {
            seen_types: HashMap<String, DescriptorProto>,
            f: &'s dyn Fn(&mut Box<TypeCollector>, &DescriptorProto, &str) -> Vec<DescriptorProto>,
        }

        let mut collector = Box::new(TypeCollector {
            seen_types: HashMap::new(),
            f: &|collector: &mut Box<TypeCollector>,
                 message_type: &DescriptorProto,
                 prefix: &str| {
                let absolute_name = format!("{prefix:}.{:}", message_type.name());
                if collector.seen_types.contains_key(&absolute_name) {
                    Vec::default()
                } else {
                    {
                        let collector_mut = collector.deref_mut();
                        collector_mut
                            .seen_types
                            .insert(absolute_name.clone(), message_type.clone());
                        dbg!(&absolute_name);
                    }

                    let mut ret: Vec<DescriptorProto> = message_type
                        .nested_type
                        .iter()
                        .flat_map(|x| (collector.f)(collector, x, &absolute_name))
                        .collect();
                    ret.push(message_type.clone());
                    ret
                }
            },
        });

        let types = request
            .proto_file
            .iter()
            .flat_map(move |x| {
                let package_prefix = if let Some(package) = &x.package {
                    format!(".{package:}")
                } else {
                    "".to_string()
                };
                x.message_type
                    .iter()
                    .flat_map(|ty| (&collector.f.clone())(&mut collector, ty, &package_prefix))
                    .collect::<Vec<DescriptorProto>>()
            })
            .collect::<Vec<DescriptorProto>>();
        for ty in types {
            dbg!(ty.name);
        }
        Self {
            protoc_version,
            request,
        }
    }

    pub fn generate(&self) -> CodeGeneratorResponse {
        let files: Result<Vec<code_generator_response::File>, CodeGeneratorException> = self
            .request
            .proto_file
            .iter()
            .map(|file| self.generate_file(file))
            .try_collect();

        if let Err(err) = files {
            CodeGeneratorResponse {
                error: Some(format!("{err:?}")),
                file: Vec::default(),
                supported_features: None,
                special_fields: SpecialFields::default(),
            }
        } else {
            CodeGeneratorResponse {
                error: None,
                supported_features: None,
                file: files.unwrap(),
                special_fields: SpecialFields::default(),
            }
        }
    }

    pub fn generate_file(
        &self,
        file: &FileDescriptorProto,
    ) -> Result<code_generator_response::File, CodeGeneratorException> {
        todo!()
    }
}
