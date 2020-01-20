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
};

// Each node takes up eight consecutive slots in tree[], which correspond to the eight child pointers.
// The first 31 bits are the pointer, the last bit is set if it's a non-leaf voxel.
// So, an empty leaf voxel is 0.
layout(set=0, binding=0) buffer octree_buffer {
    uint tree[];
};

#include "sky.glsl"
#include "octree.glsl"
#include "shade.glsl"


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

/*
vec3 shade(vec3 rd, vec3 normal, MatData mat, vec3 pos) {
    float sun_speed = TAU * SUN_SPEED;
    vec3 sun_dir = vec3(sin(iTime * sun_speed), cos(iTime * sun_speed), 0.0);

    vec3 sun_color = pow(vec3(0.7031,0.4687,0.1055), vec3(1.0 / 4.2));
    vec3 sky_color = pow(vec3(0.3984,0.5117,0.7305), vec3(1.0 / 4.2));

    float sha = 1.0;//shadow(pos, normal, sun_dir);

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
*/

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

  vec2 t;
  int i = 256;
  vec3 p;
  if (trace(ro, rd, t, i, p)) {
    MatData mat;
    mat.color = vec3(0.3, 0.6, 0.1);
    frag_color = vec4(shade(ro, rd, t, p), 1.0);
  } else {
    frag_color = vec4(sky(ro, rd), 1.0);
  }
}
