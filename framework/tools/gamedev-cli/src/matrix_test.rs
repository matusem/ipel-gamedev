#[cfg(test)]
mod matrix {
    use crate::cli::{BackendKind, FrontendKind};

    #[test]
    fn implemented_backend_frontend_matrix_is_documented() {
        let backends = [BackendKind::Rust, BackendKind::Java];
        let frontends = [FrontendKind::Js, FrontendKind::Ts, FrontendKind::Bevy];
        for b in backends {
            assert!(b.is_implemented(), "{b:?} should be implemented");
            for f in frontends {
                assert!(f.is_implemented(), "{f:?} should be implemented");
            }
        }
    }

    #[test]
    fn scaffold_templates_exist() {
        let templates = [
            "templates/backend/rust_shared_types_lib.rs",
            "templates/backend/rust_shared_types_export_schema.rs",
            "templates/frontend/play_main.js",
            "templates/frontend/play_main.ts",
            "templates/frontend/bevy_main.rs",
            "templates/client/index.html",
        ];
        for t in templates {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(t);
            assert!(path.is_file(), "missing template {}", path.display());
        }
    }
}
