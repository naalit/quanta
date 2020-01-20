#define BEVEL 0

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

vec3 shade(in vec3 ro, in vec3 rd, in vec2 t, in vec3 pos) {
    vec3 p = ro+rd*t.x;
    vec3 n = p-pos;

    //n = sign(n) * vec3(all(greaterThan(n.xx, n.yz)), all(greaterThan(n.yy,n.xz)), all(greaterThan(n.zz,n.yx)));

#if BEVEL
    n = normalize(sign(n) * pow(abs(n), vec3(3)));
#else
    n = sign(n) * (abs(n.x) > abs(n.y) ? // Not y
        (abs(n.x) > abs(n.z) ? vec3(1., 0., 0.) : vec3(0., 0., 1.)) :
    	(abs(n.y) > abs(n.z) ? vec3(0., 1., 0.) : vec3(0., 0., 1.)));
#endif

    vec3 light = normalize(vec3(-0.2,0.8,0.3));

    float occ = ao(floor(p-0.1*n), n, p);

    return vec3(1.0) * max(0.0,dot(light,n)) * (0.5 * occ) + 0.2 * occ;
}
