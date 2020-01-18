#version 450

layout(location=0) in vec2 frag_coord_ndc;
layout(location=0) out vec4 frag_color;

layout(push_constant) uniform PushConstants {
  float fov;
  vec2 resolution;
  // Origin of the chunks texture, in chunks
  ivec3 start;
  vec3 camera_pos;
  vec3 camera_dir;
  vec3 camera_up;
};
// A chunk is 14x14, but we store a 1-voxel lip around it so it takes up 16x16 of texture space
// chunks stores texel offsets of 16x16x64 chunks - 16^3 for each cascade - after the lip
// An offset of (8192, 8192, 8192) is an empty chunk
layout(set=0,binding=0) uniform usampler3D chunks;
layout(set=0,binding=1) uniform sampler3D blocks;


ivec3 world_to_chunk(vec3 w) {
  // a = if w < 0.0 { 1 } else { 0 }
  ivec3 a = 1 - ivec3(step(0.0, w));
  return (ivec3(w)) / 14 - a;
}
vec3 chunk_to_world(ivec3 c) {
  return (vec3(c) + 0.5) * 14.0;
}
vec3 pos_in_chunk(vec3 w) {
  return mod(mod(w, 14.0) + 14.0, 14.0);
}


bool trace(in vec3 ro, in vec3 rd, in float tanW, out float t) {
  vec3 p = ro;
  vec3 i_rd = 1.0 / rd;
  t = 0.0;
  ivec3 chunk = world_to_chunk(p) - start;
  vec3 offset = vec3(texelFetch(chunks, chunk, 0).xyz);

  vec3 in_chunk = pos_in_chunk(p);
  vec3 i_size = 1.0 / vec3(textureSize(blocks, 0).xyz);

  float candidate_t = 0.2;
  float candidate_dt = 1000000.0;

  for (int it = 0; it < 256; it++) {
    float d = texture(blocks, (offset + in_chunk) * i_size).r * 16.0 - 2.0;
    t += d;
    if (d <= 0.01)//t * tanW)
      return true;
    in_chunk += rd * d;

    if (d / t < candidate_dt) {
      candidate_dt = d / t;
      candidate_t = t;
    }

    // Maybe advance chunk
    if (any(greaterThanEqual(in_chunk, vec3(14.0))) || any(lessThan(in_chunk, vec3(0.0)))) {
      do {
        // Intersect ray with this chunk
        vec3 c = chunk_to_world(chunk + start);
        vec3 mn = c - 7.0;
        vec3 mx = c + 7.0;
        vec3 t1 = (mn-ro) * i_rd;
        vec3 t2 = (mx-ro) * i_rd;
        vec3 tmax = max(t1, t2);

        t = 0.001 + min(tmax.x, min(tmax.y, tmax.z));
        p = ro + t * rd;
        chunk = world_to_chunk(p) - start;

        if (any(greaterThanEqual(chunk, ivec3(16))) || any(lessThan(chunk, ivec3(0))))
          return false;

        offset = vec3(texelFetch(chunks, chunk, 0).xyz);
        // Skip empty chunks, but avoid looping infinitely
      } while (offset.x == 8192 && it++ < 256);
      in_chunk = pos_in_chunk(p);
    }
  }
  t = candidate_t;
  return true;
}

bool trace_(in vec3 ro, in vec3 rd, in float tanW, out float t) {
  t = 0;
  for (int it = 0; it < 128; it++) {
    float d = texture(blocks, ro + t * rd).r;//length(ro + t * rd - vec3(0.0, 0.0, 2.0)) - 1.0;
    t += d;
    if (d <= 0.01) {
      return true;
    }
  }
  return false;
}

vec3 normal(in vec3 p) {
  ivec3 chunk = world_to_chunk(p) - start;
  vec3 offset = vec3(texelFetch(chunks, chunk, 0).xyz);
  vec3 in_chunk = pos_in_chunk(p);
  vec3 i_size = 1.0 / vec3(textureSize(blocks, 0).xyz);
  p = (offset + in_chunk) * i_size;

  vec2 e = vec2(0.0, 0.001);
  return normalize(vec3(
    texture(blocks, p + e.yxx).r - texture(blocks, p - e.yxx).r,
    texture(blocks, p + e.xyx).r - texture(blocks, p - e.xyx).r,
    texture(blocks, p + e.xxy).r - texture(blocks, p - e.xxy).r
  ));
}

void main() {
  vec2 uv = frag_coord_ndc;
  uv.y *= resolution.y / resolution.x;
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

  float w = acos(dot(rd, camera_dir));
  w = abs(dFdy(w));
  float tanW = tan(w);

  float t;
  if (trace(ro, rd, tanW, t)) {
    vec3 l = normalize(vec3(0.2, 1.0, 0.1));
    float c = dot(normal(ro + rd * t), l);
    frag_color = vec4(c, c, c, 1.0);
  } else {
    frag_color = vec4(0.0, 0.0, t/16.0, 1.0);
  }
}
