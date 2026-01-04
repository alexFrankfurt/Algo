//! Build script to compile GLSL shaders to SPIR-V
//! Requires glslc (from Vulkan SDK) to be in PATH

use std::process::Command;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src/shaders/");
    
    let shaders = [
        ("src/shaders/task.glsl", "src/shaders/task.spv", "task"),
        ("src/shaders/mesh.glsl", "src/shaders/mesh.spv", "mesh"),
        ("src/shaders/frag.glsl", "src/shaders/frag.spv", "frag"),
        ("src/shaders/background.vert.glsl", "src/shaders/background.vert.spv", "vert"),
        ("src/shaders/background.frag.glsl", "src/shaders/background.frag.spv", "background_frag"),
    ];
    
    for (input, output, shader_type) in shaders {
        if !Path::new(input).exists() {
            panic!("Shader source not found: {}", input);
        }
        
        let stage = match shader_type {
            "task" => "task",
            "mesh" => "mesh", 
            "frag" => "frag",
            "vert" => "vert",
            "background_frag" => "frag",
            _ => panic!("Unknown shader type"),
        };
        
        let status = Command::new("glslc")
            .args([
                "--target-env=vulkan1.3",
                &format!("-fshader-stage={}", stage),
                "-o", output,
                input,
            ])
            .status()
            .expect("Failed to run glslc. Make sure Vulkan SDK is installed.");
        
        if !status.success() {
            panic!("Failed to compile shader: {}", input);
        }
        
        println!("cargo:warning=Compiled {} -> {}", input, output);
    }
}
