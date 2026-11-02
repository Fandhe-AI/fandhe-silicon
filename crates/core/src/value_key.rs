//! 値を指す鍵の型と、上限の値の鍵の静的な定数と一覧（TASK-1362）。
//!
//! 値の鍵の型 `ValueKey` は `FeatureId` と別の型で、上限の値の鍵は 57 件ある。背後実装の型は含まない。feature の条件なしで公開される。
//!
//! 鍵の文字列は `limit:` に続けて小文字の名前を並べる。

/// 値を指す鍵。`FeatureId` と別の型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueKey(pub &'static str);

/// `limit:max_texture_dimension_1d` の値の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_1D: ValueKey = ValueKey("limit:max_texture_dimension_1d");

/// `limit:max_texture_dimension_2d` の値の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_2D: ValueKey = ValueKey("limit:max_texture_dimension_2d");

/// `limit:max_texture_dimension_3d` の値の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_3D: ValueKey = ValueKey("limit:max_texture_dimension_3d");

/// `limit:max_texture_array_layers` の値の鍵。
pub const LIMIT_MAX_TEXTURE_ARRAY_LAYERS: ValueKey = ValueKey("limit:max_texture_array_layers");

/// `limit:max_bind_groups` の値の鍵。
pub const LIMIT_MAX_BIND_GROUPS: ValueKey = ValueKey("limit:max_bind_groups");

/// `limit:max_bind_groups_plus_vertex_buffers` の値の鍵。
pub const LIMIT_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS: ValueKey =
    ValueKey("limit:max_bind_groups_plus_vertex_buffers");

/// `limit:max_bindings_per_bind_group` の値の鍵。
pub const LIMIT_MAX_BINDINGS_PER_BIND_GROUP: ValueKey =
    ValueKey("limit:max_bindings_per_bind_group");

/// `limit:max_dynamic_uniform_buffers_per_pipeline_layout` の値の鍵。
pub const LIMIT_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT: ValueKey =
    ValueKey("limit:max_dynamic_uniform_buffers_per_pipeline_layout");

/// `limit:max_dynamic_storage_buffers_per_pipeline_layout` の値の鍵。
pub const LIMIT_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT: ValueKey =
    ValueKey("limit:max_dynamic_storage_buffers_per_pipeline_layout");

/// `limit:max_sampled_textures_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_sampled_textures_per_shader_stage");

/// `limit:max_samplers_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_SAMPLERS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_samplers_per_shader_stage");

/// `limit:max_storage_buffers_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_storage_buffers_per_shader_stage");

/// `limit:max_storage_textures_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_storage_textures_per_shader_stage");

/// `limit:max_uniform_buffers_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_uniform_buffers_per_shader_stage");

/// `limit:max_binding_array_elements_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_ELEMENTS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_binding_array_elements_per_shader_stage");

/// `limit:max_binding_array_acceleration_structure_elements_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_binding_array_acceleration_structure_elements_per_shader_stage");

/// `limit:max_binding_array_sampler_elements_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_SAMPLER_ELEMENTS_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_binding_array_sampler_elements_per_shader_stage");

/// `limit:max_uniform_buffer_binding_size` の値の鍵。
pub const LIMIT_MAX_UNIFORM_BUFFER_BINDING_SIZE: ValueKey =
    ValueKey("limit:max_uniform_buffer_binding_size");

/// `limit:max_storage_buffer_binding_size` の値の鍵。
pub const LIMIT_MAX_STORAGE_BUFFER_BINDING_SIZE: ValueKey =
    ValueKey("limit:max_storage_buffer_binding_size");

/// `limit:max_vertex_buffers` の値の鍵。
pub const LIMIT_MAX_VERTEX_BUFFERS: ValueKey = ValueKey("limit:max_vertex_buffers");

/// `limit:max_buffer_size` の値の鍵。
pub const LIMIT_MAX_BUFFER_SIZE: ValueKey = ValueKey("limit:max_buffer_size");

/// `limit:max_vertex_attributes` の値の鍵。
pub const LIMIT_MAX_VERTEX_ATTRIBUTES: ValueKey = ValueKey("limit:max_vertex_attributes");

/// `limit:max_vertex_buffer_array_stride` の値の鍵。
pub const LIMIT_MAX_VERTEX_BUFFER_ARRAY_STRIDE: ValueKey =
    ValueKey("limit:max_vertex_buffer_array_stride");

/// `limit:max_inter_stage_shader_variables` の値の鍵。
pub const LIMIT_MAX_INTER_STAGE_SHADER_VARIABLES: ValueKey =
    ValueKey("limit:max_inter_stage_shader_variables");

/// `limit:min_uniform_buffer_offset_alignment` の値の鍵。
pub const LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT: ValueKey =
    ValueKey("limit:min_uniform_buffer_offset_alignment");

/// `limit:min_storage_buffer_offset_alignment` の値の鍵。
pub const LIMIT_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT: ValueKey =
    ValueKey("limit:min_storage_buffer_offset_alignment");

/// `limit:max_color_attachments` の値の鍵。
pub const LIMIT_MAX_COLOR_ATTACHMENTS: ValueKey = ValueKey("limit:max_color_attachments");

/// `limit:max_color_attachment_bytes_per_sample` の値の鍵。
pub const LIMIT_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE: ValueKey =
    ValueKey("limit:max_color_attachment_bytes_per_sample");

/// `limit:max_compute_workgroup_storage_size` の値の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE: ValueKey =
    ValueKey("limit:max_compute_workgroup_storage_size");

/// `limit:max_compute_invocations_per_workgroup` の値の鍵。
pub const LIMIT_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP: ValueKey =
    ValueKey("limit:max_compute_invocations_per_workgroup");

/// `limit:max_compute_workgroup_size_x` の値の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X: ValueKey =
    ValueKey("limit:max_compute_workgroup_size_x");

/// `limit:max_compute_workgroup_size_y` の値の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Y: ValueKey =
    ValueKey("limit:max_compute_workgroup_size_y");

/// `limit:max_compute_workgroup_size_z` の値の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Z: ValueKey =
    ValueKey("limit:max_compute_workgroup_size_z");

/// `limit:max_compute_workgroups_per_dimension` の値の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION: ValueKey =
    ValueKey("limit:max_compute_workgroups_per_dimension");

/// `limit:max_immediate_size` の値の鍵。
pub const LIMIT_MAX_IMMEDIATE_SIZE: ValueKey = ValueKey("limit:max_immediate_size");

/// `limit:max_non_sampler_bindings` の値の鍵。
pub const LIMIT_MAX_NON_SAMPLER_BINDINGS: ValueKey = ValueKey("limit:max_non_sampler_bindings");

/// `limit:max_task_workgroup_total_count` の値の鍵。
pub const LIMIT_MAX_TASK_WORKGROUP_TOTAL_COUNT: ValueKey =
    ValueKey("limit:max_task_workgroup_total_count");

/// `limit:max_task_workgroups_per_dimension` の値の鍵。
pub const LIMIT_MAX_TASK_WORKGROUPS_PER_DIMENSION: ValueKey =
    ValueKey("limit:max_task_workgroups_per_dimension");

/// `limit:max_mesh_workgroup_total_count` の値の鍵。
pub const LIMIT_MAX_MESH_WORKGROUP_TOTAL_COUNT: ValueKey =
    ValueKey("limit:max_mesh_workgroup_total_count");

/// `limit:max_mesh_workgroups_per_dimension` の値の鍵。
pub const LIMIT_MAX_MESH_WORKGROUPS_PER_DIMENSION: ValueKey =
    ValueKey("limit:max_mesh_workgroups_per_dimension");

/// `limit:max_task_invocations_per_workgroup` の値の鍵。
pub const LIMIT_MAX_TASK_INVOCATIONS_PER_WORKGROUP: ValueKey =
    ValueKey("limit:max_task_invocations_per_workgroup");

/// `limit:max_task_invocations_per_dimension` の値の鍵。
pub const LIMIT_MAX_TASK_INVOCATIONS_PER_DIMENSION: ValueKey =
    ValueKey("limit:max_task_invocations_per_dimension");

/// `limit:max_mesh_invocations_per_workgroup` の値の鍵。
pub const LIMIT_MAX_MESH_INVOCATIONS_PER_WORKGROUP: ValueKey =
    ValueKey("limit:max_mesh_invocations_per_workgroup");

/// `limit:max_mesh_invocations_per_dimension` の値の鍵。
pub const LIMIT_MAX_MESH_INVOCATIONS_PER_DIMENSION: ValueKey =
    ValueKey("limit:max_mesh_invocations_per_dimension");

/// `limit:max_task_payload_size` の値の鍵。
pub const LIMIT_MAX_TASK_PAYLOAD_SIZE: ValueKey = ValueKey("limit:max_task_payload_size");

/// `limit:max_mesh_output_vertices` の値の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_VERTICES: ValueKey = ValueKey("limit:max_mesh_output_vertices");

/// `limit:max_mesh_output_primitives` の値の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_PRIMITIVES: ValueKey = ValueKey("limit:max_mesh_output_primitives");

/// `limit:max_mesh_output_layers` の値の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_LAYERS: ValueKey = ValueKey("limit:max_mesh_output_layers");

/// `limit:max_mesh_multiview_view_count` の値の鍵。
pub const LIMIT_MAX_MESH_MULTIVIEW_VIEW_COUNT: ValueKey =
    ValueKey("limit:max_mesh_multiview_view_count");

/// `limit:max_blas_primitive_count` の値の鍵。
pub const LIMIT_MAX_BLAS_PRIMITIVE_COUNT: ValueKey = ValueKey("limit:max_blas_primitive_count");

/// `limit:max_blas_geometry_count` の値の鍵。
pub const LIMIT_MAX_BLAS_GEOMETRY_COUNT: ValueKey = ValueKey("limit:max_blas_geometry_count");

/// `limit:max_tlas_instance_count` の値の鍵。
pub const LIMIT_MAX_TLAS_INSTANCE_COUNT: ValueKey = ValueKey("limit:max_tlas_instance_count");

/// `limit:max_acceleration_structures_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_ACCELERATION_STRUCTURES_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_acceleration_structures_per_shader_stage");

/// `limit:max_buffers_and_acceleration_structures_per_shader_stage` の値の鍵。
pub const LIMIT_MAX_BUFFERS_AND_ACCELERATION_STRUCTURES_PER_SHADER_STAGE: ValueKey =
    ValueKey("limit:max_buffers_and_acceleration_structures_per_shader_stage");

/// `limit:max_multiview_view_count` の値の鍵。
pub const LIMIT_MAX_MULTIVIEW_VIEW_COUNT: ValueKey = ValueKey("limit:max_multiview_view_count");

/// `limit:max_ray_dispatch_count` の値の鍵。
pub const LIMIT_MAX_RAY_DISPATCH_COUNT: ValueKey = ValueKey("limit:max_ray_dispatch_count");

/// `limit:max_ray_recursion_depth` の値の鍵。
pub const LIMIT_MAX_RAY_RECURSION_DEPTH: ValueKey = ValueKey("limit:max_ray_recursion_depth");

/// 上限の値の鍵の一覧。`limit_key::LIMIT_QUERY_KEYS` と同じ並びで、同じ位置の鍵は同じ文字列を持つ。
///
/// ```
/// use fandhe_silicon_core::value_key::LIMIT_VALUE_KEYS;
///
/// assert_eq!(LIMIT_VALUE_KEYS.len(), 57);
/// assert_eq!(
///     fandhe_silicon_core::value_key::LIMIT_MAX_BUFFER_SIZE.0,
///     fandhe_silicon_core::limit_key::LIMIT_MAX_BUFFER_SIZE.0,
/// );
/// ```
pub const LIMIT_VALUE_KEYS: &[ValueKey] = &[
    LIMIT_MAX_TEXTURE_DIMENSION_1D,
    LIMIT_MAX_TEXTURE_DIMENSION_2D,
    LIMIT_MAX_TEXTURE_DIMENSION_3D,
    LIMIT_MAX_TEXTURE_ARRAY_LAYERS,
    LIMIT_MAX_BIND_GROUPS,
    LIMIT_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS,
    LIMIT_MAX_BINDINGS_PER_BIND_GROUP,
    LIMIT_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT,
    LIMIT_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT,
    LIMIT_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE,
    LIMIT_MAX_SAMPLERS_PER_SHADER_STAGE,
    LIMIT_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE,
    LIMIT_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE,
    LIMIT_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE,
    LIMIT_MAX_BINDING_ARRAY_ELEMENTS_PER_SHADER_STAGE,
    LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE,
    LIMIT_MAX_BINDING_ARRAY_SAMPLER_ELEMENTS_PER_SHADER_STAGE,
    LIMIT_MAX_UNIFORM_BUFFER_BINDING_SIZE,
    LIMIT_MAX_STORAGE_BUFFER_BINDING_SIZE,
    LIMIT_MAX_VERTEX_BUFFERS,
    LIMIT_MAX_BUFFER_SIZE,
    LIMIT_MAX_VERTEX_ATTRIBUTES,
    LIMIT_MAX_VERTEX_BUFFER_ARRAY_STRIDE,
    LIMIT_MAX_INTER_STAGE_SHADER_VARIABLES,
    LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT,
    LIMIT_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT,
    LIMIT_MAX_COLOR_ATTACHMENTS,
    LIMIT_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE,
    LIMIT_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE,
    LIMIT_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP,
    LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X,
    LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Y,
    LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Z,
    LIMIT_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION,
    LIMIT_MAX_IMMEDIATE_SIZE,
    LIMIT_MAX_NON_SAMPLER_BINDINGS,
    LIMIT_MAX_TASK_WORKGROUP_TOTAL_COUNT,
    LIMIT_MAX_TASK_WORKGROUPS_PER_DIMENSION,
    LIMIT_MAX_MESH_WORKGROUP_TOTAL_COUNT,
    LIMIT_MAX_MESH_WORKGROUPS_PER_DIMENSION,
    LIMIT_MAX_TASK_INVOCATIONS_PER_WORKGROUP,
    LIMIT_MAX_TASK_INVOCATIONS_PER_DIMENSION,
    LIMIT_MAX_MESH_INVOCATIONS_PER_WORKGROUP,
    LIMIT_MAX_MESH_INVOCATIONS_PER_DIMENSION,
    LIMIT_MAX_TASK_PAYLOAD_SIZE,
    LIMIT_MAX_MESH_OUTPUT_VERTICES,
    LIMIT_MAX_MESH_OUTPUT_PRIMITIVES,
    LIMIT_MAX_MESH_OUTPUT_LAYERS,
    LIMIT_MAX_MESH_MULTIVIEW_VIEW_COUNT,
    LIMIT_MAX_BLAS_PRIMITIVE_COUNT,
    LIMIT_MAX_BLAS_GEOMETRY_COUNT,
    LIMIT_MAX_TLAS_INSTANCE_COUNT,
    LIMIT_MAX_ACCELERATION_STRUCTURES_PER_SHADER_STAGE,
    LIMIT_MAX_BUFFERS_AND_ACCELERATION_STRUCTURES_PER_SHADER_STAGE,
    LIMIT_MAX_MULTIVIEW_VIEW_COUNT,
    LIMIT_MAX_RAY_DISPATCH_COUNT,
    LIMIT_MAX_RAY_RECURSION_DEPTH,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limit_key;

    #[test]
    fn limit_value_keys_has_57_entries() {
        assert_eq!(LIMIT_VALUE_KEYS.len(), 57);
    }

    #[test]
    fn limit_value_keys_are_distinct() {
        for (i, a) in LIMIT_VALUE_KEYS.iter().enumerate() {
            for b in &LIMIT_VALUE_KEYS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn limit_value_keys_match_limit_query_keys() {
        assert_eq!(LIMIT_VALUE_KEYS.len(), limit_key::LIMIT_QUERY_KEYS.len());
        for (v, q) in LIMIT_VALUE_KEYS.iter().zip(limit_key::LIMIT_QUERY_KEYS) {
            assert_eq!(v.0, q.0);
        }
    }

    #[test]
    fn constants_match_limit_key_constants() {
        assert_eq!(
            LIMIT_MAX_TEXTURE_DIMENSION_1D.0,
            limit_key::LIMIT_MAX_TEXTURE_DIMENSION_1D.0
        );
        assert_eq!(
            LIMIT_MAX_RAY_RECURSION_DEPTH.0,
            limit_key::LIMIT_MAX_RAY_RECURSION_DEPTH.0
        );
        assert_eq!(LIMIT_MAX_BUFFER_SIZE.0, limit_key::LIMIT_MAX_BUFFER_SIZE.0);
        assert_eq!(
            LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT.0,
            limit_key::LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT.0
        );
        assert_eq!(
            LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE.0,
            limit_key::LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE.0
        );
        assert_eq!(LIMIT_VALUE_KEYS[0], LIMIT_MAX_TEXTURE_DIMENSION_1D);
        assert_eq!(
            LIMIT_VALUE_KEYS.last().copied(),
            Some(LIMIT_MAX_RAY_RECURSION_DEPTH)
        );
    }
}
