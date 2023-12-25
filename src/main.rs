use bytes::Bytes;
use itertools::{self, Itertools};
use protobuf::descriptor::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    OneofDescriptorProto, ServiceDescriptorProto,
};
use protobuf::plugin::{code_generator_response, CodeGeneratorRequest, CodeGeneratorResponse};
use protobuf::Message;
use protobuf::SpecialFields;
use std::collections::HashSet;
use std::io::{self, Read, Write};
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

// thank you, reru!
struct TypeCollector {
  seen: HashSet<String>,
  elements: Vec<DescriptorProto>,
}

impl TypeCollector {
  fn recurse(&mut self, mut descriptor: DescriptorProto, prefix: &str) {
      let absolute_name = format!("{prefix:}.{:}", descriptor.name());
      if self.seen.contains(&absolute_name) {
          return;
      }

      let children = std::mem::take(&mut descriptor.nested_type);

      self.seen.insert(absolute_name.clone());
      self.elements.push(descriptor);

      for child in children {
          self.recurse(child, &absolute_name);
      }
  }

  fn run(&mut self, file: FileDescriptorProto, prefix: &str) -> &[DescriptorProto] {
      self.seen.clear();
      self.elements.clear();

      for ty in file.message_type {
        self.recurse(ty, prefix);
      }

      &self.elements
  }

  fn new() -> Self {
    Self {
      seen: HashSet::new(),
      elements: Vec::new()
    }
  }
}

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

        let mut type_collector = TypeCollector::new();
        let mut types: Vec<DescriptorProto> = Vec::new();
        for file in &request.proto_file {
          let package_prefix = if let Some(package) = &file.package {
              format!(".{package:}")
          } else {
              "".to_string()
          };

          // clone is necessary here :-(
          let file_types = type_collector.run(file.clone(), &package_prefix);
          types.extend_from_slice(file_types)
        }

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
