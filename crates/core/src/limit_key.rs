//! 能力問合せの口へ渡す上限の鍵の静的な定数と、その一覧（TASK-1362）。
//!
//! 鍵は `FeatureId` の定数で、57 件ある。背後実装の型・規則は含まない。feature の条件なしで公開される。
//!
//! 鍵の文字列は `limit:` に続けて小文字の名前を並べる。

use crate::capability::FeatureId;

/// `limit:max_texture_dimension_1d` の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_1D: FeatureId = FeatureId("limit:max_texture_dimension_1d");

/// `limit:max_texture_dimension_2d` の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_2D: FeatureId = FeatureId("limit:max_texture_dimension_2d");

/// `limit:max_texture_dimension_3d` の鍵。
pub const LIMIT_MAX_TEXTURE_DIMENSION_3D: FeatureId = FeatureId("limit:max_texture_dimension_3d");

/// `limit:max_texture_array_layers` の鍵。
pub const LIMIT_MAX_TEXTURE_ARRAY_LAYERS: FeatureId = FeatureId("limit:max_texture_array_layers");

/// `limit:max_bind_groups` の鍵。
pub const LIMIT_MAX_BIND_GROUPS: FeatureId = FeatureId("limit:max_bind_groups");

/// `limit:max_bind_groups_plus_vertex_buffers` の鍵。
pub const LIMIT_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS: FeatureId =
    FeatureId("limit:max_bind_groups_plus_vertex_buffers");

/// `limit:max_bindings_per_bind_group` の鍵。
pub const LIMIT_MAX_BINDINGS_PER_BIND_GROUP: FeatureId =
    FeatureId("limit:max_bindings_per_bind_group");

/// `limit:max_dynamic_uniform_buffers_per_pipeline_layout` の鍵。
pub const LIMIT_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT: FeatureId =
    FeatureId("limit:max_dynamic_uniform_buffers_per_pipeline_layout");

/// `limit:max_dynamic_storage_buffers_per_pipeline_layout` の鍵。
pub const LIMIT_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT: FeatureId =
    FeatureId("limit:max_dynamic_storage_buffers_per_pipeline_layout");

/// `limit:max_sampled_textures_per_shader_stage` の鍵。
pub const LIMIT_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_sampled_textures_per_shader_stage");

/// `limit:max_samplers_per_shader_stage` の鍵。
pub const LIMIT_MAX_SAMPLERS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_samplers_per_shader_stage");

/// `limit:max_storage_buffers_per_shader_stage` の鍵。
pub const LIMIT_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_storage_buffers_per_shader_stage");

/// `limit:max_storage_textures_per_shader_stage` の鍵。
pub const LIMIT_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_storage_textures_per_shader_stage");

/// `limit:max_uniform_buffers_per_shader_stage` の鍵。
pub const LIMIT_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_uniform_buffers_per_shader_stage");

/// `limit:max_binding_array_elements_per_shader_stage` の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_ELEMENTS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_binding_array_elements_per_shader_stage");

/// `limit:max_binding_array_acceleration_structure_elements_per_shader_stage` の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_binding_array_acceleration_structure_elements_per_shader_stage");

/// `limit:max_binding_array_sampler_elements_per_shader_stage` の鍵。
pub const LIMIT_MAX_BINDING_ARRAY_SAMPLER_ELEMENTS_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_binding_array_sampler_elements_per_shader_stage");

/// `limit:max_uniform_buffer_binding_size` の鍵。
pub const LIMIT_MAX_UNIFORM_BUFFER_BINDING_SIZE: FeatureId =
    FeatureId("limit:max_uniform_buffer_binding_size");

/// `limit:max_storage_buffer_binding_size` の鍵。
pub const LIMIT_MAX_STORAGE_BUFFER_BINDING_SIZE: FeatureId =
    FeatureId("limit:max_storage_buffer_binding_size");

/// `limit:max_vertex_buffers` の鍵。
pub const LIMIT_MAX_VERTEX_BUFFERS: FeatureId = FeatureId("limit:max_vertex_buffers");

/// `limit:max_buffer_size` の鍵。
pub const LIMIT_MAX_BUFFER_SIZE: FeatureId = FeatureId("limit:max_buffer_size");

/// `limit:max_vertex_attributes` の鍵。
pub const LIMIT_MAX_VERTEX_ATTRIBUTES: FeatureId = FeatureId("limit:max_vertex_attributes");

/// `limit:max_vertex_buffer_array_stride` の鍵。
pub const LIMIT_MAX_VERTEX_BUFFER_ARRAY_STRIDE: FeatureId =
    FeatureId("limit:max_vertex_buffer_array_stride");

/// `limit:max_inter_stage_shader_variables` の鍵。
pub const LIMIT_MAX_INTER_STAGE_SHADER_VARIABLES: FeatureId =
    FeatureId("limit:max_inter_stage_shader_variables");

/// `limit:min_uniform_buffer_offset_alignment` の鍵。
pub const LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT: FeatureId =
    FeatureId("limit:min_uniform_buffer_offset_alignment");

/// `limit:min_storage_buffer_offset_alignment` の鍵。
pub const LIMIT_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT: FeatureId =
    FeatureId("limit:min_storage_buffer_offset_alignment");

/// `limit:max_color_attachments` の鍵。
pub const LIMIT_MAX_COLOR_ATTACHMENTS: FeatureId = FeatureId("limit:max_color_attachments");

/// `limit:max_color_attachment_bytes_per_sample` の鍵。
pub const LIMIT_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE: FeatureId =
    FeatureId("limit:max_color_attachment_bytes_per_sample");

/// `limit:max_compute_workgroup_storage_size` の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE: FeatureId =
    FeatureId("limit:max_compute_workgroup_storage_size");

/// `limit:max_compute_invocations_per_workgroup` の鍵。
pub const LIMIT_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP: FeatureId =
    FeatureId("limit:max_compute_invocations_per_workgroup");

/// `limit:max_compute_workgroup_size_x` の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X: FeatureId =
    FeatureId("limit:max_compute_workgroup_size_x");

/// `limit:max_compute_workgroup_size_y` の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Y: FeatureId =
    FeatureId("limit:max_compute_workgroup_size_y");

/// `limit:max_compute_workgroup_size_z` の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_Z: FeatureId =
    FeatureId("limit:max_compute_workgroup_size_z");

/// `limit:max_compute_workgroups_per_dimension` の鍵。
pub const LIMIT_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION: FeatureId =
    FeatureId("limit:max_compute_workgroups_per_dimension");

/// `limit:max_immediate_size` の鍵。
pub const LIMIT_MAX_IMMEDIATE_SIZE: FeatureId = FeatureId("limit:max_immediate_size");

/// `limit:max_non_sampler_bindings` の鍵。
pub const LIMIT_MAX_NON_SAMPLER_BINDINGS: FeatureId = FeatureId("limit:max_non_sampler_bindings");

/// `limit:max_task_workgroup_total_count` の鍵。
pub const LIMIT_MAX_TASK_WORKGROUP_TOTAL_COUNT: FeatureId =
    FeatureId("limit:max_task_workgroup_total_count");

/// `limit:max_task_workgroups_per_dimension` の鍵。
pub const LIMIT_MAX_TASK_WORKGROUPS_PER_DIMENSION: FeatureId =
    FeatureId("limit:max_task_workgroups_per_dimension");

/// `limit:max_mesh_workgroup_total_count` の鍵。
pub const LIMIT_MAX_MESH_WORKGROUP_TOTAL_COUNT: FeatureId =
    FeatureId("limit:max_mesh_workgroup_total_count");

/// `limit:max_mesh_workgroups_per_dimension` の鍵。
pub const LIMIT_MAX_MESH_WORKGROUPS_PER_DIMENSION: FeatureId =
    FeatureId("limit:max_mesh_workgroups_per_dimension");

/// `limit:max_task_invocations_per_workgroup` の鍵。
pub const LIMIT_MAX_TASK_INVOCATIONS_PER_WORKGROUP: FeatureId =
    FeatureId("limit:max_task_invocations_per_workgroup");

/// `limit:max_task_invocations_per_dimension` の鍵。
pub const LIMIT_MAX_TASK_INVOCATIONS_PER_DIMENSION: FeatureId =
    FeatureId("limit:max_task_invocations_per_dimension");

/// `limit:max_mesh_invocations_per_workgroup` の鍵。
pub const LIMIT_MAX_MESH_INVOCATIONS_PER_WORKGROUP: FeatureId =
    FeatureId("limit:max_mesh_invocations_per_workgroup");

/// `limit:max_mesh_invocations_per_dimension` の鍵。
pub const LIMIT_MAX_MESH_INVOCATIONS_PER_DIMENSION: FeatureId =
    FeatureId("limit:max_mesh_invocations_per_dimension");

/// `limit:max_task_payload_size` の鍵。
pub const LIMIT_MAX_TASK_PAYLOAD_SIZE: FeatureId = FeatureId("limit:max_task_payload_size");

/// `limit:max_mesh_output_vertices` の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_VERTICES: FeatureId = FeatureId("limit:max_mesh_output_vertices");

/// `limit:max_mesh_output_primitives` の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_PRIMITIVES: FeatureId =
    FeatureId("limit:max_mesh_output_primitives");

/// `limit:max_mesh_output_layers` の鍵。
pub const LIMIT_MAX_MESH_OUTPUT_LAYERS: FeatureId = FeatureId("limit:max_mesh_output_layers");

/// `limit:max_mesh_multiview_view_count` の鍵。
pub const LIMIT_MAX_MESH_MULTIVIEW_VIEW_COUNT: FeatureId =
    FeatureId("limit:max_mesh_multiview_view_count");

/// `limit:max_blas_primitive_count` の鍵。
pub const LIMIT_MAX_BLAS_PRIMITIVE_COUNT: FeatureId = FeatureId("limit:max_blas_primitive_count");

/// `limit:max_blas_geometry_count` の鍵。
pub const LIMIT_MAX_BLAS_GEOMETRY_COUNT: FeatureId = FeatureId("limit:max_blas_geometry_count");

/// `limit:max_tlas_instance_count` の鍵。
pub const LIMIT_MAX_TLAS_INSTANCE_COUNT: FeatureId = FeatureId("limit:max_tlas_instance_count");

/// `limit:max_acceleration_structures_per_shader_stage` の鍵。
pub const LIMIT_MAX_ACCELERATION_STRUCTURES_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_acceleration_structures_per_shader_stage");

/// `limit:max_buffers_and_acceleration_structures_per_shader_stage` の鍵。
pub const LIMIT_MAX_BUFFERS_AND_ACCELERATION_STRUCTURES_PER_SHADER_STAGE: FeatureId =
    FeatureId("limit:max_buffers_and_acceleration_structures_per_shader_stage");

/// `limit:max_multiview_view_count` の鍵。
pub const LIMIT_MAX_MULTIVIEW_VIEW_COUNT: FeatureId = FeatureId("limit:max_multiview_view_count");

/// `limit:max_ray_dispatch_count` の鍵。
pub const LIMIT_MAX_RAY_DISPATCH_COUNT: FeatureId = FeatureId("limit:max_ray_dispatch_count");

/// `limit:max_ray_recursion_depth` の鍵。
pub const LIMIT_MAX_RAY_RECURSION_DEPTH: FeatureId = FeatureId("limit:max_ray_recursion_depth");

/// 上限の問合せの鍵の一覧。鍵は互いに異なる。
///
/// # Examples
///
/// ```
/// use fandhe_silicon_core::limit_key::{LIMIT_QUERY_KEYS, LIMIT_MAX_BUFFER_SIZE};
///
/// assert_eq!(LIMIT_QUERY_KEYS.len(), 57);
/// assert!(LIMIT_QUERY_KEYS.contains(&LIMIT_MAX_BUFFER_SIZE));
/// assert_eq!(LIMIT_MAX_BUFFER_SIZE.0, "limit:max_buffer_size");
/// ```
pub const LIMIT_QUERY_KEYS: &[FeatureId] = &[
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

    #[test]
    fn limit_query_keys_has_57_entries() {
        assert_eq!(LIMIT_QUERY_KEYS.len(), 57);
    }

    #[test]
    fn limit_query_keys_are_distinct() {
        for (i, a) in LIMIT_QUERY_KEYS.iter().enumerate() {
            for b in &LIMIT_QUERY_KEYS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn limit_query_keys_have_prefix_and_name() {
        for key in LIMIT_QUERY_KEYS {
            let rest = key.0.strip_prefix("limit:").expect("prefix");
            assert!(!rest.is_empty());
            assert!(
                rest.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            );
        }
    }

    #[test]
    fn constant_names_match_strings() {
        assert_eq!(LIMIT_QUERY_KEYS[0], LIMIT_MAX_TEXTURE_DIMENSION_1D);
        assert_eq!(
            LIMIT_QUERY_KEYS.last().copied(),
            Some(LIMIT_MAX_RAY_RECURSION_DEPTH)
        );
        assert_eq!(
            LIMIT_MAX_BINDING_ARRAY_ACCELERATION_STRUCTURE_ELEMENTS_PER_SHADER_STAGE.0,
            "limit:max_binding_array_acceleration_structure_elements_per_shader_stage"
        );
        assert_eq!(
            LIMIT_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT.0,
            "limit:min_uniform_buffer_offset_alignment"
        );
        assert_eq!(
            LIMIT_MAX_COMPUTE_WORKGROUP_SIZE_X.0,
            "limit:max_compute_workgroup_size_x"
        );
    }
}
