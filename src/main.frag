#version 450

#define saturate(x) clamp(x, 0.0, 1.0)
#define IPI 3.1415926535

layout(location=0) in vec2 frag_coord_ndc;
layout(location=0) out vec4 frag_color;

layout(push_constant) uniform PushConstants {
  float fov;
  vec2 resolution;
  vec3 origin;
  float root_size;
  vec3 camera_pos;
  vec3 camera_dir;
  vec3 camera_up;
  vec3 sun_dir;
};

// Each node takes up eight consecutive slots in tree[], which correspond to the eight child pointers.
// The first 31 bits are the pointer, the last bit is set if it's a non-leaf voxel.
// So, an empty leaf voxel is 0.
layout(set=0, binding=0, std430) buffer octree_buffer {
  uint tree[];
};
layout(set=0, binding=1) uniform sampler2D beam_image;

#define MAX_ITER 256

#include "sky.glsl"
#include "octree.glsl"
#include "shade.glsl"

layout(set=0, binding=2, std430) buffer material_buffer {
  MatData mats[];
};

void main() {
  vec2 uv = frag_coord_ndc;
  vec4 ts = textureGather(beam_image, uv*0.5+0.5);
  float start_t = max(0.0, min(min(ts.x, ts.y), min(ts.z, ts.w)) - 1.0);
  uv.x *= resolution.x / resolution.y;
  // Vulkan has this backwards for us
  uv *= -1;

  // Circle in the center of the screen to show where they're pointing
  if (length(uv) < 0.006 && length(uv) > 0.005) {
      frag_color = vec4(1.0);
      return;
  }

  vec3 right = normalize(cross(camera_up, camera_dir));
  vec3 ro = camera_pos;

  vec3 rd = camera_dir;
  float film_width = tan(fov*0.5);
  rd += film_width * camera_up * uv.y;
  rd += film_width * right * uv.x;
  rd = normalize(rd);

  ro += rd * start_t;

  vec2 t;
  int i = 256;
  vec3 p;
  uint result = trace(ro, rd, t, i, p);
  if (result != 0) {
    MatData mat = mats[result];
    //mat.color = vec3(0.3, 0.6, 0.1);
    frag_color = vec4(shade(ro, rd, t, p, mat), 1.0);
  } else {
    frag_color = vec4(sky(ro, rd), 1.0);
  }
  // frag_color.r = float(i)/256.0;
}
