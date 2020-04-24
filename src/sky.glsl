float isect(vec3 ro, vec3 rd, vec3 p, float size) {
    vec3 mn = p - size;
    vec3 mx = p + size;
    vec3 t1 = (mn-ro) / rd;
    vec3 t2 = (mx-ro) / rd;
    vec3 tmin = min(t1, t2);
    vec3 tmax = max(t1, t2);
	  vec2 t = vec2(max(tmin.x, max(tmin.y, tmin.z)), min(tmax.x, min(tmax.y, tmax.z)));
    return (t.y > t.x) && (t.y > 0.0) ? clamp((t.y-t.x) * 0.05, 0.0, 1.0) : 0.0;
}

vec3 sky(vec3 ro, vec3 rd)
{
    float sun = isect(ro, rd, ro + sun_dir * 1000.0, 40.0);
    sun = sun * length(sun) * 10.0;
    sun += 0.5 * isect(ro, rd, ro + sun_dir * 1000.0, 80.0);

    vec3 col = vec3(sun) * pow(vec3(0.7031,0.4687,0.1055), vec3(1.2) * (2.0 - sun_dir.y))
		+ 0.8 * vec3(0.3984,0.5117,0.7305) * ((0.5 + 1.0 * pow(sun_dir.y,0.4)) * (1.2-0.7*dot(vec3(0,1,0), rd)));

    return col;
}
