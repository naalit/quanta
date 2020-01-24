//#define STACKLESS
#define START_OUTSIDE

#define PRE_ITER 2

uint u_idx(vec3 idx) {
    return 0u
        | uint(idx.x > 0.0) << 2
        | uint(idx.y > 0.0) << 1
        | uint(idx.z > 0.0);
}
bvec3 b_idx(vec3 idx) {
    return greaterThan(idx, vec3(0));
}
vec3 v_idx(bvec3 b) {
  return vec3(b) * 2.0 - 1.0;
}

#ifndef STACKLESS
const int MAX_LEVELS = 8;

struct ST {
    uint parent_pointer;
    vec3 pos;
    bvec3 idx;
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
    bvec3 idx;
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

#ifdef TAN_W
uint trace(in vec3 ro, in vec3 rd, in float tan_w, out vec2 t, out int i, out vec3 pos) {
#else
uint trace(in vec3 ro, in vec3 rd, out vec2 t, inout int i, out vec3 pos) {
#endif
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
    if (t.x > t.y || t.y <= 0.0) return 0;// else return true;
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

    for (; i > 0; i--) {
        t = isect(ro, rdi, pos, size, tmid, tmax);

        uidx = u_idx(idx);

        uint node = tree[parent_pointer + uidx];

        #ifdef TAN_W
        if (size * 0.5 > abs(t.x) * tan_w && (node & 1u) > 0) {
        #else
        if ((node & 1u) > 0) { // Non-leaf
        #endif
            if (c) {
              //-- PUSH --//
              #ifndef STACKLESS
              if (t.y < h)
                  stack_push(ST(parent_pointer, pos, b_idx(idx), size, h));
              #endif
              h = t.y;
              parent_pointer += node >> 1;
              size *= 0.5;
              // Which axes we're skipping the first voxel on (hitting it from the side)
              q = lessThanEqual(tmid, vec3(t.x));
              idx = mix(-tstep, tstep, q);
              // tmax of the resulting voxel
              tmax = mix(tmid, tmax, q);
              // Don't worry about voxels behind `ro`
              idx = mix(-idx, idx, greaterThanEqual(tmax, vec3(0)));
              pos += 0.5 * size * idx;
              continue;
            }
        } else if (node != 0) // Nonempty, but either leaf, or TAN_W and it's small enough
            return node >> 1;

        //-- ADVANCE --//

        // Advance for every direction where we're hitting the side
        vec3 old = idx;
        idx = mix(idx, tstep, equal(tmax, vec3(t.y)));
        pos += mix(vec3(0.0), tstep, notEqual(old, idx)) * size;

        if (old == idx) { // We're at the last child
            //-- POP --//
            #ifdef STACKLESS

            vec3 target = pos;
            size = root_size;
            pos = root_pos;

            t = isect(ro, rdi, pos, size, tmid, tmax);
            if (t.y <= h)
                return 0;

            parent_pointer = 0;
            float nh = t.y;
            for (int j = 0; j < 100; j++) { // J is there just in case
                size *= 0.5;
                idx = sign(target - pos);
                if (any(equal(idx, vec3(0.0))))
                  break;
                pos += idx * size * 0.5;
                t = isect(ro, rdi, pos, size, tmid, tmax);

                // We have more nodes to traverse within this one
                if (t.y > h) {
                    uidx = u_idx(idx);
                    node = tree[parent_pointer + uidx];
                    parent_pointer += node >> 1;
                    nh = t.y;
                } else break;
            }
            h = nh;

            #else
            if (stack_empty()) return 0;

            ST s = stack_pop();
            h = s.h;
            idx = v_idx(s.idx);
            parent_pointer = s.parent_pointer;
            pos = s.pos;
            size = s.size;

            #endif

            c = false;
            continue;
        }
        c = true;
    }

    #ifdef TAN_W
    return 1;
    #else
    return 0;
    #endif
}
