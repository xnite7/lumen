//! Head-tracked 3D: stereo (side-by-side / over-under) films turned into a
//! window you can look around.
//!
//! A 3D release carries a left-eye and a right-eye picture. An mpv user
//! shader (three passes, all on the GPU):
//!   1. matches the two pictures at low resolution to find how far each
//!      patch shifts between the eyes (disparity = depth),
//!   2. smooths that depth map, keeping edges where the picture has edges,
//!   3. renders one picture from a virtual camera that slides between (and a
//!      little past) the two real ones, following your head.
//! `head` and `strength` are libplacebo DYNAMIC params, so moving your head
//! updates the picture every frame without recompiling anything.

/// `layout` is "sbs" (left|right) or "ou" (left over right).
pub fn shader(layout: &str) -> String {
    let ou = layout == "ou";
    // view `v` (0 = left, 1 = right) at view coordinates `u` -> frame coords
    let view = if ou {
        "vec2 vpos(vec2 u, float v) { return vec2(u.x, (clamp(u.y, 0.0, 1.0) + v) * 0.5); }"
    } else {
        "vec2 vpos(vec2 u, float v) { return vec2((clamp(u.x, 0.0, 1.0) + v) * 0.5, u.y); }"
    };
    // output size of one view
    let (vw, vh) = if ou { ("HOOKED.w", "HOOKED.h 2 /") } else { ("HOOKED.w 2 /", "HOOKED.h") };
    // disparity search runs at 1/4 of a view
    let (dw, dh) = if ou { ("HOOKED.w 4 /", "HOOKED.h 8 /") } else { ("HOOKED.w 8 /", "HOOKED.h 4 /") };

    format!(
        r#"// couch: head-tracked 3D ({layout})

//!PARAM head
//!DESC virtual camera position: -1 far left .. 1 far right
//!TYPE DYNAMIC float
//!MINIMUM -1.0
//!MAXIMUM 1.0
0.0

//!PARAM strength
//!DESC how far past the real cameras the view may go
//!TYPE DYNAMIC float
//!MINIMUM 0.0
//!MAXIMUM 2.0
1.0

//!HOOK MAIN
//!BIND HOOKED
//!SAVE COUCHDISP
//!WIDTH {dw}
//!HEIGHT {dh}
//!COMPONENTS 1
//!DESC couch 3D: depth from the two eyes

{view}
float lum(vec2 p) {{ return dot(HOOKED_tex(p).rgb, vec3(0.299, 0.587, 0.114)); }}

vec4 hook() {{
    vec2 u = HOOKED_pos;
    // one view pixel, in view coordinates
    vec2 px = vec2(1.0) / vec2({vwf}, {vhf});
    const int R = 18;          // search +-18 steps
    const float STEP = 2.0;    // of 2 view pixels: +-36 px
    float best = 1e9, bestk = 0.0, prev = 1e9, cprev = 1e9, cnext = 1e9;
    float costs[2 * R + 1];
    for (int k = -R; k <= R; k++) {{
        float s = float(k) * STEP;
        float c = 0.0;
        for (int j = -1; j <= 1; j++)
        for (int i = -1; i <= 1; i++) {{
            vec2 o = vec2(float(i), float(j)) * px * 3.0;
            float l = lum(vpos(u + o, 0.0));
            float r = lum(vpos(u + o - vec2(s * px.x, 0.0), 1.0));
            c += abs(l - r);
        }}
        costs[k + R] = c;
        if (c < best) {{ best = c; bestk = float(k); }}
    }}
    // sub-step refinement: fit a parabola through the best and its neighbours
    int b = int(bestk) + R;
    float d = bestk;
    if (b > 0 && b < 2 * R) {{
        float a0 = costs[b - 1], a1 = costs[b], a2 = costs[b + 1];
        float den = a0 - 2.0 * a1 + a2;
        if (den > 1e-5) d += clamp(0.5 * (a0 - a2) / den, -0.5, 0.5);
    }}
    // flat, featureless patches can't be matched: trust them less
    float conf = clamp((costs[0] + costs[2 * R]) * 0.5 - best, 0.0, 1.0);
    return vec4(d * STEP * px.x * conf, 0.0, 0.0, 1.0);
}}

//!HOOK MAIN
//!BIND HOOKED
//!BIND COUCHDISP
//!SAVE COUCHDEPTH
//!WIDTH COUCHDISP.w
//!HEIGHT COUCHDISP.h
//!COMPONENTS 1
//!DESC couch 3D: smooth depth, keep edges

{view}
vec4 hook() {{
    vec2 u = COUCHDISP_pos;
    vec2 t = COUCHDISP_pt;
    float c = COUCHDISP_tex(u).x;
    vec3 g = HOOKED_tex(vpos(u, 0.0)).rgb;
    float sum = 0.0, wsum = 0.0;
    for (int j = -2; j <= 2; j++)
    for (int i = -2; i <= 2; i++) {{
        vec2 q = u + vec2(float(i), float(j)) * t;
        float v = COUCHDISP_tex(q).x;
        vec3 gq = HOOKED_tex(vpos(q, 0.0)).rgb;
        float w = exp(-dot(gq - g, gq - g) * 40.0) * exp(-float(i * i + j * j) * 0.18);
        sum += v * w;
        wsum += w;
    }}
    // a median-ish pull toward the centre value removes speckles
    float s = sum / max(wsum, 1e-4);
    return vec4(mix(s, c, 0.15), 0.0, 0.0, 1.0);
}}

//!HOOK MAIN
//!BIND HOOKED
//!BIND COUCHDEPTH
//!WIDTH {vw}
//!HEIGHT {vh}
//!DESC couch 3D: the view from where your head is

{view}
vec4 hook() {{
    // a touch of zoom hides the strip the far edge can't fill
    vec2 u = (HOOKED_pos - 0.5) / (1.0 + 0.035 * strength) + 0.5;
    float d = COUCHDEPTH_tex(u).x;
    // a = 0 left camera, 1 right camera; past either end is extrapolated
    float a = 0.5 + head * (0.5 + 0.55 * strength);
    vec4 L = HOOKED_tex(vpos(u + vec2(a * d, 0.0), 0.0));
    vec4 R = HOOKED_tex(vpos(u - vec2((1.0 - a) * d, 0.0), 1.0));
    return mix(L, R, clamp(a, 0.0, 1.0));
}}
"#,
        layout = layout,
        view = view,
        dw = dw,
        dh = dh,
        vw = vw,
        vh = vh,
        vwf = if ou { "HOOKED_size.x" } else { "HOOKED_size.x * 0.5" },
        vhf = if ou { "HOOKED_size.y * 0.5" } else { "HOOKED_size.y" },
    )
}
