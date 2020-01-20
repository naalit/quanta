#define STACKLESS
#define START_OUTSIDE

#define PRE_ITER 2

#define MAX_ITER 256

uint u_idx(vec3 idx) {
    return 0u
        | uint(idx.x > 0.0) << 2
        | uint(idx.y > 0.0) << 1
        | uint(idx.z > 0.0);
}

#ifndef STACKLESS
const int MAX_LEVELS = 8;

struct ST {
    uint parent_pointer;
    vec3 pos;
    vec3 idx;
    float size;
    float h;
} stack[MAX_LEVELS];

int stack_ptr = 0; // Next open index
void stack_reset() { stack_ptr = 0; }
void stack_push(in ST s) { stack[stack_ptr++] = s; }
ST stack_pop() { return stack[--stack_ptr]; }
bool stack_empty() { return stack_ptr == 0; }
#else
struct ST {
    uint parent_pointer;
    vec3 pos;
    vec3 idx;
    float size;
    float h;
};
#endif

// `rdi` is 1/rd, assumed to have been precomputed
vec2 isect(in vec3 ro, in vec3 rdi, in vec3 pos, in float size, out vec3 tmid, out vec3 tmax) {
    vec3 mn = pos - 0.5 * size;
    vec3 mx = mn + size;
    vec3 t1 = (mn-ro) * rdi;
    vec3 t2 = (mx-ro) * rdi;
    vec3 tmin = min(t1, t2);
    tmax = max(t1, t2);

    tmid = (pos-ro) * rdi;

    return vec2(max(tmin.x, max(tmin.y, tmin.z)), min(tmax.x, min(tmax.y, tmax.z)));
}

uint get_voxel(in vec3 target) {
    float size = root_size;
    vec3 pos = origin;
    vec3 idx;

    uint parent_pointer = 0;
    for (int j = 0; j < 100; j++) { // J is there just in case
        size *= 0.5;
        idx = sign(target-pos+0.0001); // Add 0.0001 to avoid zeros
        pos += idx * size * 0.5;

        uint uidx = u_idx(idx);
        uint node = tree[parent_pointer + uidx];

        // We have more nodes to traverse within this one
        if ((node & 1u) > 0) {
            parent_pointer += node >> 1;
        } else return node;
    }
    return 0u;
}

bool trace(in vec3 ro, in vec3 rd, out vec2 t, out int i, out vec3 pos) {
    #ifndef STACKLESS
    stack_reset();
    #endif

    vec3 tstep = sign(rd);
    vec3 rdi = 1.0 / rd; // Inverse for isect

    //float root_size = 8.0;
    vec3 root_pos = origin;//vec3(2,-2,4);
    pos = root_pos;

    vec3 tmid, tmax;
    t = isect(ro, rdi, pos, root_size, tmid, tmax);
    if (t.x > t.y || t.y <= 0.0) return false;// else return true;
    float h = t.y;

    // If the minimum is before the middle in this axis, we need to go to the first one (-rd)
    //vec3 idx = mix(-tstep, tstep, lessThanEqual(tmid, vec3(t.x)));
    #ifdef START_OUTSIDE
    bvec3 q = lessThanEqual(tmid, vec3(t.x));
    vec3 idx = mix(-tstep, tstep, q);
    vec3 tq = mix(tmid, tmax, q); // tmax of the resulting voxel
    idx = mix(-idx, idx, greaterThanEqual(tq, vec3(0))); // Don't worry about voxels behind `ro`
    uint uidx;
    float size = root_size * 0.5;
    pos += 0.5 * size * idx;
    uint parent_pointer = 0;

    #else

    bvec3 q;
    vec3 idx;
    vec3 tq;
    uint uidx;
    float size = root_size;
    uint parent_pointer = 0;

    vec3 target = ro+max(0.0,t.x)*rd;
    for (int j = 0; j < PRE_ITER; j++) { // J is there just in case
        size *= 0.5;
        idx = sign(target-pos+0.0001); // Add 0.0001 to avoid zeros
        pos += idx * size * 0.5;

        uidx = u_idx(idx);
        uint node = tree[parent_pointer + uidx];

        // We have more nodes to traverse within this one
        if ((node & 1u) > 0) {
            parent_pointer += node >> 1;
        } else break;
    }
    t = isect(ro, rdi, pos, size, tmid, tmax);
    h = t.y;
    #endif

    bool c = true;
    ST s = ST(parent_pointer,pos,idx,size,h);

    for (i = 0; i < MAX_ITER; i++) {
        t = isect(ro, rdi, s.pos, s.size, tmid, tmax);

        uidx = u_idx(s.idx);

        uint node = tree[s.parent_pointer + uidx];

        if ((node & 1u) > 0) { // Non-leaf
            if (c) {
                //-- PUSH --//
                #ifndef STACKLESS
                if (t.y < s.h)
                    stack_push(s);
                #endif
                s.h = t.y;
                s.parent_pointer += node >> 1;
                s.size *= 0.5;
                q = lessThanEqual(tmid, vec3(t.x));
                s.idx = mix(-tstep, tstep, q);
                tq = mix(tmid, tmax, q); // tmax of the resulting voxel
                s.idx = mix(-s.idx, s.idx, greaterThanEqual(tq, vec3(0))); // Don't worry about voxels behind `ro`
                s.pos += 0.5 * s.size * s.idx;
                continue;
            }
        } else if (node != 0) { // Nonempty, but leaf
            pos = s.pos;
            return true;
        }

        //-- ADVANCE --//

        // Advance for every direction where we're hitting the side
        vec3 old = s.idx;
        s.idx = mix(s.idx, tstep, equal(tmax, vec3(t.y)));
        s.pos += mix(vec3(0.0), tstep, notEqual(old, s.idx)) * s.size;

        if (old == s.idx) { // We're at the last child
            //-- POP --//
            //continue;
            // return true;
            #ifdef STACKLESS

            vec3 target = s.pos;
            s.size = root_size;
            s.pos = root_pos;

            t = isect(ro,rdi,s.pos,s.size,tmid,tmax);
            if (t.y <= s.h)
                return false;

            s.parent_pointer = 0;
            float nh = t.y;
            for (int j = 0; j < 100; j++) { // J is there just in case
                s.size *= 0.5;
                s.idx = sign(target-s.pos+0.0001); // Add 0.0001 to avoid zeros
                s.pos += s.idx * s.size * 0.5;
                t = isect(ro, rdi, s.pos, s.size, tmid, tmax);

                // We have more nodes to traverse within this one
                if (t.y > s.h) {
                    uidx = u_idx(s.idx);
                    node = tree[s.parent_pointer + uidx];
                    s.parent_pointer += node >> 1;
                    nh = t.y;
                } else break;
            }
            s.h = nh;

            #else
            if (stack_empty()) return false;

            s = stack_pop();
            #endif

            c = false;
            continue;
        }
        c = true;

    }

    return false;
}
