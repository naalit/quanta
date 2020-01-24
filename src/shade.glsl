#define BEVEL 0
#define SHADOWS 1
#define SHADOW_ITERS 64

struct MatData {
    vec3 color;
    float roughness;
    float trans;
    float metal;
    float ior;
    float nothing; // Just buffer to pack it in right
};

bool map(in vec3 p) { return get_voxel(p+0.5) > 0; }

// From reinder - https://www.shadertoy.com/view/4ds3WS
vec4 edges( in vec3 vos, in vec3 nor, in vec3 dir )
{
	vec3 v1 = vos + nor + dir.yzx;
	vec3 v2 = vos + nor - dir.yzx;
	vec3 v3 = vos + nor + dir.zxy;
	vec3 v4 = vos + nor - dir.zxy;

	vec4 res = vec4(0.0);
	if( map(v1) ) res.x = 1.0;
	if( map(v2) ) res.y = 1.0;
	if( map(v3) ) res.z = 1.0;
	if( map(v4) ) res.w = 1.0;

	return res;
}

vec4 corners( in vec3 vos, in vec3 nor, in vec3 dir )
{
	vec3 v1 = vos + nor + dir.yzx + dir.zxy;
	vec3 v2 = vos + nor - dir.yzx + dir.zxy;
	vec3 v3 = vos + nor - dir.yzx - dir.zxy;
	vec3 v4 = vos + nor + dir.yzx - dir.zxy;

	vec4 res = vec4(0.0);
	if( map(v1) ) res.x = 1.0;
	if( map(v2) ) res.y = 1.0;
	if( map(v3) ) res.z = 1.0;
	if( map(v4) ) res.w = 1.0;

	return res;
}

float ao(in vec3 vos, in vec3 nor, in vec3 pos) {
    vec3 dir = abs(nor);

    vec4 ed = edges( vos, nor, dir );
    vec4 co = corners( vos, nor, dir );
    vec3 uvw = pos - vos;
    vec2 uv = vec2( dot(dir.yzx, uvw), dot(dir.zxy, uvw) );
    float occ = 0.0;
    // (for edges)
    occ += (    uv.x) * ed.x;
    occ += (1.0-uv.x) * ed.y;
    occ += (    uv.y) * ed.z;
    occ += (1.0-uv.y) * ed.w;
    // (for corners)
    occ += (      uv.y *     uv.x ) * co.x*(1.0-ed.x)*(1.0-ed.z);
    occ += (      uv.y *(1.0-uv.x)) * co.y*(1.0-ed.z)*(1.0-ed.y);
    occ += ( (1.0-uv.y)*(1.0-uv.x)) * co.z*(1.0-ed.y)*(1.0-ed.w);
    occ += ( (1.0-uv.y)*     uv.x ) * co.w*(1.0-ed.w)*(1.0-ed.x);
    occ = 1.0 - occ/8.0;
    occ = occ*occ;
    occ = occ*occ;

    return occ;
}

#if SHADOWS
float shadow(in vec3 p, in vec3 sun_dir, in vec3 normal) {
	vec2 t;
	int i = SHADOW_ITERS;
	vec3 pos;
	return float(trace(p+normal*0.01, sun_dir, t, i, pos) == 0);
}
#endif

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

vec3 shade(in vec3 ro, in vec3 rd, in vec2 t, in vec3 pos, in MatData mat) {
    vec3 p = ro+rd*t.x;
    vec3 n = p-pos;

#if BEVEL
    n = normalize(sign(n) * pow(abs(n), vec3(3)));
#else
    n = sign(n) * (abs(n.x) > abs(n.y) ? // Not y
        (abs(n.x) > abs(n.z) ? vec3(1., 0., 0.) : vec3(0., 0., 1.)) :
    	(abs(n.y) > abs(n.z) ? vec3(0., 1., 0.) : vec3(0., 0., 1.)));
#endif

#if SHADOWS
		float sha = shadow(p, sun_dir, n);
#else
	  float sha = 1.0;
#endif

    float occ = ao(floor(p-0.1*n), n, p);

		vec3 sun_color = pow(vec3(0.7031,0.4687,0.1055), vec3(1.0 / 4.2));
		vec3 sky_color = pow(vec3(0.3984,0.5117,0.7305), vec3(1.0 / 4.2));

		vec3 col = sha * 0.5 * sun_color * smoothstep(0.0, 0.1, sun_dir.y) * saturate(dot(n, sun_dir));//saturate(bsdf(-rd, sun_dir, normal, mat));
		col += occ * sky_color * 0.2 * saturate(0.5 + 0.5*n.y + 0.2*n.x);//mat.color * IPI * length(abs(normal) * vec3(0.7, 1.0, 0.85));//bsdf(-rd, normalize(normal * vec3(1, 0, 1)), normal, mat);
		col += occ * pow(sun_color, vec3(1.2)) * 0.2 * smoothstep(0.0, 0.1, sun_dir.y) * saturate(dot(n, normalize(sun_dir * vec3(-1,0,-1))));//saturate(bsdf(-rd, -sun_dir, normal, mat));

		col *= IPI * pow(mat.color, vec3(2.2));
		// if (mat.roughness < 0.2) {
		//     vec3 r = reflect(rd,normal);
		//     col += 0.25 * sky(pos, r) * min(vec3(1.0),bsdf(-rd, normalize(r), normal, mat));
		// }
		col = applyFog(col, length(pos-camera_pos), camera_pos, rd, sun_dir);
		return col;
}
