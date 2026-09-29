//! Shared by the shader tests: translate a validated module the way wgpu
//! does on macOS (Metal Shading Language) and on Windows without DXC (HLSL,
//! shader model 5.1 for FXC). naga's writers reject constructs those
//! backends cannot express, so this catches them on a Linux machine.

pub fn translate_for_every_backend(module: &naga::Module, info: &naga::valid::ModuleInfo) -> Result<(), String> {
    // wgpu resolves `override` constants (to their defaults here) before any
    // backend sees the module
    let (module, info) = naga::back::pipeline_constants::process_overrides(
        module,
        info,
        None,
        &naga::back::PipelineConstants::default(),
    )
    .map_err(|e| format!("pipeline constants: {e}"))?;
    let (module, info) = (module.as_ref(), info.as_ref());
    let msl = naga::back::msl::Options {
        // macOS 12+ devices; wgpu asks the device for newer when available
        lang_version: (2, 4),
        ..Default::default()
    };
    naga::back::msl::write_string(module, info, &msl, &naga::back::msl::PipelineOptions::default())
        .map_err(|e| format!("Metal (MSL): {e}"))?;
    let hlsl = naga::back::hlsl::Options {
        shader_model: naga::back::hlsl::ShaderModel::V5_1,
        ..Default::default()
    };
    let mut out = String::new();
    naga::back::hlsl::Writer::new(&mut out, &hlsl, &naga::back::hlsl::PipelineOptions::default())
        .write(module, info, None)
        .map_err(|e| format!("DX12 (HLSL SM 5.1): {e}"))?;
    Ok(())
}
