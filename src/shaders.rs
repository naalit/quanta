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

mod beam {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "src/beam.frag"
    }
}

pub use beam::ty::PushConstants as BeamConstants;
pub use beam::Shader as Beam;
pub use fs::ty::MatData;
pub use fs::ty::PushConstants;
pub use fs::Shader as Fragment;
pub use vs::Shader as Vertex;
