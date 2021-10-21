use crate::inspector::handlers::node::base::handle_base_property_changed;
use crate::{do_command, inspector::SenderHelper, scene::commands::mesh::*};
use rg3d::{
    core::pool::Handle,
    gui::message::{CollectionChanged, FieldKind, PropertyChanged},
    scene::{mesh::Mesh, node::Node},
};

pub fn handle_mesh_property_changed(
    args: &PropertyChanged,
    handle: Handle<Node>,
    node: &Node,
    helper: &SenderHelper,
) -> Option<()> {
    match args.value {
        FieldKind::Object(ref value) => match args.name.as_ref() {
            Mesh::CAST_SHADOWS => {
                do_command!(helper, SetMeshCastShadowsCommand, handle, value)
            }
            Mesh::RENDER_PATH => {
                do_command!(helper, SetMeshRenderPathCommand, handle, value)
            }
            Mesh::DECAL_LAYER_INDEX => {
                do_command!(helper, SetMeshDecalLayerIndexCommand, handle, value)
            }
            _ => println!("Unhandled property of Sprite: {:?}", args),
        },
        FieldKind::Collection(ref args) => match **args {
            CollectionChanged::Add => {}
            CollectionChanged::Remove(_) => {}
            CollectionChanged::ItemChanged { .. } => {}
        },
        FieldKind::Inspectable(ref inner) => {
            if let Mesh::BASE = args.name.as_ref() {
                handle_base_property_changed(&inner, handle, node, helper)?
            }
        }
    }

    Some(())
}
