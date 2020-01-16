mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "src/blank.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/main.frag"
    }
}

pub use fs::ty::PushConstants;
pub use fs::Shader as Fragment;
pub use vs::Shader as Vertex;
