fn main() {
  tonic_build::compile_protos("proto/camden.proto")
    .unwrap_or_else(|e| panic!("Failed to compile protos {e:?}"));
  tonic_build::compile_protos("proto/tmf.proto")
    .unwrap_or_else(|e| panic!("Failed to compile protos {e:?}"));
}
