#version 450

#define saturate(x) clamp(x, 0.0, 1.0)
#define IPI 3.1415926535

layout(location=0) in vec2 frag_coord_ndc;
layout(location=0) out float frag_color;

layout(push_constant) uniform PushConstants {
  float fov;
  vec2 resolution;
  vec3 origin;
  float root_size;
  vec3 camera_pos;
  vec3 camera_dir;
  vec3 camera_up;
};

// Each node takes up eight consecutive slots in tree[], which correspond to the eight child pointers.
// The first 31 bits are the pointer, the last bit is set if it's a non-leaf voxel.
// So, an empty leaf voxel is 0.
layout(set=0, binding=0, std430) buffer octree_buffer {
    uint tree[];
};

#define TAN_W
#include "octree.glsl"

void main() {
  vec2 uv = frag_coord_ndc;
  uv.x *= resolution.x / resolution.y;
  // Vulkan has this backwards for us
  uv *= -1;

  vec3 right = normalize(cross(camera_up, camera_dir));
  vec3 ro = camera_pos;

  vec3 rd = camera_dir;
  float film_width = tan(fov*0.5);
  rd += film_width * camera_up * uv.y;
  rd += film_width * right * uv.x;
  rd = normalize(rd);

  float tanW = (tan(fov) / resolution.y);

  vec2 t;
  int i = 64;
  vec3 p;
  if (trace(ro, rd, tanW, t, i, p) != 0)
    frag_color = max(0.0, t.x);
  else
    frag_color = max(0.0, t.y);
}
