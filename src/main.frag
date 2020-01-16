#version 450

layout(location=0) in vec2 frag_coord_ndc;
layout(location=0) out vec4 frag_color;

const float FOV = radians(90.0);
const float FOV_MUL = tan(FOV*0.5);

layout(push_constant) uniform PushConstants {
  vec2 resolution;
  vec3 camera_pos;
  vec3 camera_dir;
  vec3 camera_up;
};
// A chunk is 14x14, but we store a 1-voxel lip around it so it takes up 16x16 of texture space
// chunks stores texel offsets of 16x16x64 chunks - 16^3 for each cascade - after the lip
layout(set=0,binding=0) uniform usampler3D chunks;
layout(set=0,binding=1) uniform sampler3D blocks;


uvec3 world_to_chunk(vec3 w) {
  // a = if w < 0.0 { 1 } else { 0 }
  uvec3 a = uvec3((sign(w)-1.0)*0.5);
  return (uvec3(w) + a) / 14 + a;
}
vec3 chunk_to_world(uvec3 c) {
  return (vec3(c) + 0.5) * 14.0;
}
vec3 pos_in_chunk(vec3 w) {
  return mod(mod(w, 14.0) + 14.0, 14.0);
}


bool trace_(in vec3 ro, in vec3 rd, in float tanW, out float t) {
  vec3 p = ro;
  vec3 i_rd = 1.0 / rd;
  t = 0.0;
  uvec3 chunk = world_to_chunk(p);
  vec3 offset = vec3(texelFetch(chunks, ivec3(chunk), 0).xyz);

  vec3 in_chunk = pos_in_chunk(p);
  float i_size = 1.0 / float(textureSize(blocks, 0).x);

  for (int it = 0; it < 128; it++) {
    float d = texture(blocks, (offset + in_chunk) * i_size).r;
    t += d;
    if (d < t * tanW)
      return true;
    in_chunk += rd * d;

    // Maybe advance chunk
    if (any(greaterThanEqual(in_chunk, vec3(14.0))) || any(lessThan(in_chunk, vec3(0.0)))) {
      // Intersect ray with this chunk
      vec3 c = chunk_to_world(chunk);
      vec3 mn = c - 7.0;
      vec3 mx = c + 7.0;
      vec3 t1 = (mn-c) * i_rd;
      vec3 t2 = (mx-c) * i_rd;
      vec3 tmax = max(t1, t2);

      t = 0.01 + min(tmax.x, min(tmax.y, tmax.z));
      p = ro + t * rd;
      chunk = world_to_chunk(p);

      if (any(greaterThanEqual(chunk, uvec3(16))))
        return false;

      offset = vec3(texelFetch(chunks, ivec3(chunk), 0).xyz);
      in_chunk = pos_in_chunk(p);
    }
  }
  return false;
}

bool trace(in vec3 ro, in vec3 rd, in float tanW, out float t) {
  t = 0;
  for (int it = 0; it < 128; it++) {
    float d = length(ro + t * rd - vec3(0.0, 0.0, 2.0)) - 1.0;
    t += d;
    if (d <= 0.01) {
      return true;
    }
  }
  return false;
}

void main() {
  vec2 uv = frag_coord_ndc;
  uv.y *= resolution.y / resolution.x;

  // Circle in the center of the screen to show where they're pointing
  if (length(uv) < 0.006 && length(uv) > 0.005) {
      frag_color = vec4(1.0);
      return;
  }

  vec3 right = normalize(cross(camera_dir, camera_up));
  vec3 ro = camera_pos;

  vec3 rd = camera_dir;
  rd += FOV_MUL * camera_up * uv.y;
  rd += FOV_MUL * right * uv.x;
  rd = normalize(rd);

  float w = acos(dot(rd, camera_dir));
  w = dFdy(w);
  float tanW = tan(w);

  float t;
  if (trace(ro, rd, tanW, t)) {
    frag_color = vec4(1.0, t, t, 1.0);
  } else {
    frag_color = vec4(0.0, 0.0, 0.0, 1.0);
  }
}
