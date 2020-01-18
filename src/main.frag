#version 450

#define saturate(x) clamp(x, 0.0, 1.0)
#define IPI 3.1415926535
//#define SOFT_SHADOWS
#include "sky.glsl"

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


bool trace(in vec3 ro, in vec3 rd, in float tanW, out float t, inout int it, out float candidate_dt) {
  vec3 p = ro;
  vec3 i_rd = 1.0 / rd;
  t = 0.0;
  ivec3 chunk = world_to_chunk(p) - start;
  vec3 offset = vec3(texelFetch(chunks, chunk, 0).xyz);

  vec3 in_chunk = pos_in_chunk(p);
  vec3 i_size = 1.0 / vec3(textureSize(blocks, 0).xyz);

  float candidate_t = 0.2;
  candidate_dt = 1000000.0;

  for (; it > 0; it--) {
    float d = texture(blocks, (offset + in_chunk) * i_size).r * 16.0 - 2.0;
    t += d;

    if (d / t < candidate_dt) {
      candidate_dt = d / t;
      candidate_t = t;
    }

    if (d <= 0.01)//t * tanW)
      return true;
    in_chunk += rd * d;

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
      } while (offset.x == 8192 && it-- > 0);
      in_chunk = pos_in_chunk(p);
    }
  }
  t = candidate_t;
  return true;
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

float shadow(in vec3 p, in vec3 normal, in vec3 sun_dir) {
  float t, dt;
  int it = 64;
  bool b = trace(p + normal * 0.1, sun_dir, 0.01, t, it, dt);
  return float(!b)
    #ifdef SOFT_SHADOWS
    * smoothstep(0.002, 0.5, dt)
    #endif
    ;
}

struct MatData {
    vec3 color;
    float roughness;
    float trans;
    float metal;
    float ior;
    float nothing; // Just buffer to pack it in right
};

// From IQ: https://iquilezles.org/www/articles/fog/fog.htm
vec3 applyFog( in vec3 rgb, // original color of the pixel
               in float dist, // camera to point distance
               in vec3 rayOri, // camera position
               in vec3 rayDir, // camera to point vector
               in vec3 sunDir ) { // sun light direction
    float c = 0.008; // Overall fog density
    float b = 0.1; // Altitude falloff
    // float c = a/b;

    float fogAmount = c * exp(-rayOri.y*b) * (1.0-exp( -dist*rayDir.y*b ))/rayDir.y;
    float sunAmount = max( dot( rayDir, sunDir ), 0.0 );
    vec3 fogColor = mix( vec3(0.5,0.6,0.7), // bluish
        vec3(1.0,0.9,0.7), // yellowish
        pow(sunAmount,8.0) );
    return mix( rgb, fogColor, fogAmount );
}

vec3 shade(vec3 rd, vec3 normal, MatData mat, vec3 pos) {
    float sun_speed = TAU * SUN_SPEED;
    vec3 sun_dir = vec3(sin(iTime * sun_speed), cos(iTime * sun_speed), 0.0);

    vec3 sun_color = pow(vec3(0.7031,0.4687,0.1055), vec3(1.0 / 4.2));
    vec3 sky_color = pow(vec3(0.3984,0.5117,0.7305), vec3(1.0 / 4.2));

    float sha = shadow(pos, normal, sun_dir);

    vec3 col = sha * sun_color * smoothstep(0.0, 0.1, sun_dir.y) * saturate(dot(normal, sun_dir));//saturate(bsdf(-rd, sun_dir, normal, mat));
    col += sky_color * 0.2 * saturate(0.5 + 0.5*normal.y + 0.2*normal.x);//mat.color * IPI * length(abs(normal) * vec3(0.7, 1.0, 0.85));//bsdf(-rd, normalize(normal * vec3(1, 0, 1)), normal, mat);
    col += sha * pow(sun_color, vec3(1.2)) * 0.2 * smoothstep(0.0, 0.1, sun_dir.y) * saturate(dot(normal, normalize(sun_dir * vec3(-1,0,-1))));//saturate(bsdf(-rd, -sun_dir, normal, mat));

    col *= IPI * mat.color;
    // if (mat.roughness < 0.2) {
    //     vec3 r = reflect(rd,normal);
    //     col += 0.25 * sky(pos, r) * min(vec3(1.0),bsdf(-rd, normalize(r), normal, mat));
    // }
    col = applyFog(col, length(pos-camera_pos), camera_pos, rd, sun_dir);
    return col;
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

  float t, _dt;
  int i = 256;
  if (trace(ro, rd, tanW, t, i, _dt)) {
    vec3 p = vec3(ro + rd * t);
    MatData mat;
    mat.color = vec3(0.3, 0.6, 0.1);
    frag_color = vec4(shade(rd, normal(p), mat, p), 1.0);
  } else {
    frag_color = vec4(sky(ro, rd), 1.0);
  }
}
