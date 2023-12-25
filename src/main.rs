use anyhow::{bail, Error, Result};
use bytes::Bytes;
use itertools::{self, Itertools};
use protobuf::descriptor::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    OneofDescriptorProto, ServiceDescriptorProto,
};
use protobuf::plugin::{code_generator_response, CodeGeneratorRequest, CodeGeneratorResponse};
use protobuf::well_known_types::type_::Enum;
use protobuf::SpecialFields;
use protobuf::{Message, MessageField};
use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use thiserror::Error;

fn main() {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut stdout = io::stdout();
    let mut stdout_handle = stdout.lock();
    if let Ok(request) = CodeGeneratorRequest::parse_from_reader(&mut handle) {
        let code_generator = CodeGenerator::new(&request);
        let result = code_generator.generate();
        let bytes = result.write_to_bytes().unwrap();
        stdout_handle.write(&bytes).unwrap();
    }
}

#[derive(Error, Debug)]
enum CodeGeneratorException {
    #[error("Unsupported protoc version - expected {expected:}, got {found:}")]
    UnsupportedProtocVersion { expected: String, found: String },
}

// thank you, reru!
struct TypeCollector {
    elements: HashMap<String, DescriptorProto>,
    enums: HashMap<String, EnumDescriptorProto>,
}

impl TypeCollector {
    fn recurse(&mut self, mut descriptor: DescriptorProto, prefix: &str) {
        let absolute_name = format!("{prefix:}.{:}", descriptor.name());
        if self.elements.contains_key(&absolute_name) {
            return;
        }

        let children = std::mem::take(&mut descriptor.nested_type);
        let enums = std::mem::take(&mut descriptor.enum_type);

        self.elements.insert(absolute_name.clone(), descriptor);

        for e in enums {
            let enum_name = format!("{absolute_name:}.{:}", e.name());
            self.enums.insert(enum_name, e);
        }

        for child in children {
            self.recurse(child, &absolute_name);
        }
    }

    fn run(
        &mut self,
        file: FileDescriptorProto,
        prefix: &str,
    ) -> (
        HashMap<String, DescriptorProto>,
        HashMap<String, EnumDescriptorProto>,
    ) {
        self.elements.clear();
        self.enums.clear();

        for ty in file.message_type {
            self.recurse(ty, prefix);
        }

        for en in file.enum_type {
            let enum_name = format!("{prefix:}.{:}", en.name());
            self.enums.insert(enum_name, en);
        }

        (self.elements.to_owned(), self.enums.to_owned())
    }

    fn new() -> Self {
        Self {
            elements: HashMap::new(),
            enums: HashMap::new(),
        }
    }
}

#[derive(Debug, Default)]
struct CodeGenerator<'a> {
    protoc_version: String,
    request: &'a CodeGeneratorRequest,
    types: HashMap<String, DescriptorProto>,
    enums: HashMap<String, EnumDescriptorProto>,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(request: &'a CodeGeneratorRequest) -> Self {
        let protoc_version = match &request.compiler_version.0 {
            Some(compiler_version) => format!(
                "{:?}{:?}{:?}",
                compiler_version.major(),
                compiler_version.minor(),
                compiler_version.patch()
            ),
            None => "".to_string(),
        };

        let mut type_collector = TypeCollector::new();
        let mut types: HashMap<String, DescriptorProto> = HashMap::new();
        let mut enums: HashMap<String, EnumDescriptorProto> = HashMap::new();
        for file in &request.proto_file {
            let package_prefix = match &file.package {
                Some(package) => format!(".{package:}"),
                None => "".to_string(),
            };

            // clone is necessary here :-(
            let (file_types, enum_types) = type_collector.run(file.clone(), &package_prefix);
            types.extend(file_types);
            enums.extend(enum_types);
        }

        Self {
            protoc_version,
            request,
            types,
            enums,
        }
    }

    pub fn generate(&self) -> CodeGeneratorResponse {
        let files: Result<Vec<code_generator_response::File>> = self
            .request
            .proto_file
            .iter()
            // .filter(|x| x.package() != "google.protobuf")
            .map(|file| self.generate_file(file))
            .try_collect();

        match files {
            Err(err) => CodeGeneratorResponse {
                error: Some(format!("{err:}")),
                file: Vec::new(),
                supported_features: None,
                special_fields: SpecialFields::default(),
            },
            Ok(files) => CodeGeneratorResponse {
                error: None,
                file: files,
                supported_features: None,
                special_fields: SpecialFields::default(),
            },
        }
    }

    pub fn generate_file(
        &self,
        file: &FileDescriptorProto,
    ) -> Result<code_generator_response::File> {
        if file.syntax() != "proto3" {
            return Err(CodeGeneratorException::UnsupportedProtocVersion {
                expected: "proto3".to_string(),
                found: file.syntax().to_string(),
            }
            .into());
        }

        let mut content = String::new();
        content.push_str("-- Generated by the protocol buffer compiler. DO NOT EDIT!\n");
        content.push_str(&format!("-- source: {:}\n\n", file.name()));
        content.push_str("local protobuf = require(\"google/protobuf\")\n\n");

        for dependency in &file.dependency {
          let dependency_name = dependency.replace(".proto", "");
          let dependency_import_name = dependency_name.replace("/", "_");
          content.push_str(&format!("local {dependency_import_name:} = require(\"{:}\")", dependency_name));
        }

        Ok(code_generator_response::File {
            name: Some(dbg!(format!("{:}.luau", file.name()))),
            insertion_point: None,
            content: Some(content),
            generated_code_info: MessageField::default(),
            special_fields: SpecialFields::default(),
        })
    }
}
