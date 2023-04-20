//! The module responsible for batch generation for rendering optimizations.

use crate::{
    core::{
        algebra::{Matrix4, Vector3},
        math::frustum::Frustum,
        sstorage::ImmutableString,
    },
    material::SharedMaterial,
    renderer::framework::geometry_buffer::ElementRange,
    scene::{
        graph::Graph,
        mesh::{surface::SurfaceSharedData, RenderPath},
    },
};
use fxhash::{FxBuildHasher, FxHashMap, FxHasher};
use std::{
    fmt::{Debug, Formatter},
    hash::Hasher,
};

/// Observer info contains all the data, that describes an observer. It could be a real camera, light source's
/// "virtual camera" that is used for shadow mapping, etc.
pub struct ObserverInfo {
    /// World-space position of the observer.
    pub observer_position: Vector3<f32>,
    /// Location of the near clipping plane.
    pub z_near: f32,
    /// Location of the far clipping plane.
    pub z_far: f32,
    /// View matrix of the observer.
    pub view_matrix: Matrix4<f32>,
    /// Projection matrix of the observer.
    pub projection_matrix: Matrix4<f32>,
}

/// Render context is used to collect render data from the scene nodes. It provides all required information about
/// the observer (camera, light source virtual camera, etc.), that could be used for culling.
pub struct RenderContext<'a> {
    /// World-space position of the observer.
    pub observer_position: &'a Vector3<f32>,
    /// Location of the near clipping plane.
    pub z_near: f32,
    /// Location of the far clipping plane.
    pub z_far: f32,
    /// View matrix of the observer.
    pub view_matrix: &'a Matrix4<f32>,
    /// Projection matrix of the observer.
    pub projection_matrix: &'a Matrix4<f32>,
    /// Frustum of the observer, it is built using observer's view and projection matrix. Use the frustum to do
    /// frustum culling.
    pub frustum: &'a Frustum,
    /// Render data batch storage. Your scene node must write at least one surface instance here for the node to
    /// be rendered.
    pub storage: &'a mut RenderDataBatchStorage,
    /// A reference to the graph that is being rendered. Allows you to get access to other scene nodes to do
    /// some useful job.
    pub graph: &'a Graph,
    /// A name of the render pass for which the context was created for.
    pub render_pass_name: &'a ImmutableString,
}

/// A set of data of a surface for rendering.  
pub struct SurfaceInstanceData {
    /// A world matrix.
    pub world_transform: Matrix4<f32>,
    /// A set of bone matrices.
    pub bone_matrices: Vec<Matrix4<f32>>,
    /// A depth-hack value.
    pub depth_offset: f32,
    /// A set of weights for each blend shape in the surface.
    pub blend_shapes_weights: Vec<f32>,
    /// A range of elements of the instance. Allows you to draw either the full range ([`ElementRange::Full`])
    /// of the graphics primitives from the surface data or just a part of it ([`ElementRange::Specific`]).
    pub element_range: ElementRange,
}

/// A set of surface instances that share the same vertex/index data and a material.
pub struct RenderDataBatch {
    /// A pointer to shared surface data.
    pub data: SurfaceSharedData,
    /// A set of instances.
    pub instances: Vec<SurfaceInstanceData>,
    /// A material that is shared across all instances.
    pub material: SharedMaterial,
    /// Whether the batch is using GPU skinning or not.
    pub is_skinned: bool,
    /// A render path of the batch.
    pub render_path: RenderPath,
    /// A decal layer index of the batch.
    pub decal_layer_index: u8,
    sort_index: u64,
}

impl Debug for RenderDataBatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Batch {}: {} instances",
            self.data.key(),
            self.instances.len()
        )
    }
}

/// Batch storage handles batch generation for a scene before rendering. It is used to optimize
/// rendering by reducing amount of state changes of OpenGL context.
#[derive(Default)]
pub struct RenderDataBatchStorage {
    batch_map: FxHashMap<u64, usize>,
    /// A sorted list of batches.
    pub batches: Vec<RenderDataBatch>,
}

impl RenderDataBatchStorage {
    /// Creates a new render batch storage from the given graph and observer info. It "asks" every node in the
    /// graph one-by-one to give render data which is then put in the storage, sorted and ready for rendering.
    /// Frustum culling is done on scene node side ([`crate::scene::node::NodeTrait::collect_render_data`]).
    pub fn from_graph(
        graph: &Graph,
        observer_info: ObserverInfo,
        render_pass_name: ImmutableString,
    ) -> Self {
        // Aim for the worst-case scenario when every node has unique render data.
        let capacity = graph.node_count() as usize;
        let mut storage = Self {
            batch_map: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher::default()),
            batches: Vec::with_capacity(capacity),
        };

        let frustum = Frustum::from_view_projection_matrix(
            observer_info.projection_matrix * observer_info.view_matrix,
        )
        .unwrap_or_default();

        let mut ctx = RenderContext {
            observer_position: &observer_info.observer_position,
            z_near: observer_info.z_near,
            z_far: observer_info.z_far,
            view_matrix: &observer_info.view_matrix,
            projection_matrix: &observer_info.projection_matrix,
            frustum: &frustum,
            storage: &mut storage,
            graph,
            render_pass_name: &render_pass_name,
        };

        for node in graph.linear_iter() {
            node.collect_render_data(&mut ctx);
        }

        storage.sort();

        storage
    }

    /// Adds a new surface instance to the storage. The method will automatically put the instance in the appropriate
    /// batch. Batch selection is done using the material, surface data, render path, decal layer index, skinning flag.
    /// If only one of these parameters is different, then the surface instance will be put in a separate batch.
    pub fn push(
        &mut self,
        data: &SurfaceSharedData,
        material: &SharedMaterial,
        render_path: RenderPath,
        decal_layer_index: u8,
        sort_index: u64,
        instance_data: SurfaceInstanceData,
    ) {
        let is_skinned = !instance_data.bone_matrices.is_empty();

        let mut hasher = FxHasher::default();
        hasher.write_u64(material.key());
        hasher.write_u64(data.key());
        hasher.write_u8(if is_skinned { 1 } else { 0 });
        hasher.write_u8(decal_layer_index);
        hasher.write_u32(render_path as u32);
        let key = hasher.finish();

        let batch = if let Some(&batch_index) = self.batch_map.get(&key) {
            self.batches.get_mut(batch_index).unwrap()
        } else {
            self.batch_map.insert(key, self.batches.len());
            self.batches.push(RenderDataBatch {
                data: data.clone(),
                sort_index,
                instances: Default::default(),
                material: material.clone(),
                is_skinned,
                render_path,
                decal_layer_index,
            });
            self.batches.last_mut().unwrap()
        };

        batch.instances.push(instance_data)
    }

    /// Sorts the batches by their respective sort index.
    pub fn sort(&mut self) {
        self.batches.sort_unstable_by_key(|b| b.sort_index);
    }
}
