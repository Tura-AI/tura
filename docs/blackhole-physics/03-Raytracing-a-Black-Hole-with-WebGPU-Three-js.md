# Raytracing a Black Hole with WebGPU | Three.js Roadmap

Source: https://threejsroadmap.com/blog/raytracing-a-black-hole-with-webgpu

[![Three.js Roadmap Logo](https://threejsroadmap.com/_next/image?url=%2Fimages%2Fthreejs-roadmap-logo-100x100.png&w=64&q=75&dpl=dpl_6gutwXJpXam1NyMyirySGFNmw5q2)THREE.JS ROADMAP](https://threejsroadmap.com/)[Assets](https://threejsroadmap.com/assets)[Courses](https://threejsroadmap.com/courses)[Blog](https://threejsroadmap.com/blog)[FAQ](https://threejsroadmap.com/#faq)[Login](https://threejsroadmap.com/login)[Sign Up](https://threejsroadmap.com/signup)[](https://threejsroadmap.com/cart) # Raytracing a Black Hole with WebGPU

Dan Greenheck•January 7, 2026•ShadersView source> Click and drag to interact! (WebGPU-capable browser required)

[Source Code](https://github.com/dgreenheck/webgpu-black-hole) • [Live Demo](https://dgreenheck.github.io/webgpu-black-hole/)

When *Interstellar* came out in 2014, the imagery of Gargantua was striking not just because it looked cool, but because it was based on actual physics. Kip Thorne and his team ran real gravitational lensing calculations to produce those visuals.

![gargantua](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767759963539.png)Gargantua, the supermassive black hole from the movie Interstellar (2014) In this tutorial, we're going to recreate that effect in real-time using WebGPU and Three.js Shading Language (TSL). We'll cover both the physics and the implementation, so you'll understand what's happening under the hood.

### What We're Building

- A raymarched black hole with gravitational lensing
- An accretion disk with temperature-based blackbody coloring
- Doppler beaming (relativistic brightness asymmetry)
- Turbulent ring patterns with Keplerian rotation
- A procedural star field and nebula background distorted by gravity
- Interactive controls for all parameters

[Live Demo](https://threejsroadmap.com/blog/#) | [Source Code](https://github.com/dgreenheck/webgpu-black-hole)

---

## Part 1: The Physics

Before we write any code, let's understand what we're actually simulating.

### 1.1 Schwarzschild Spacetime

A black hole is a region where gravity is so strong that nothing can escape, not even light. For a non-rotating black hole (a Schwarzschild black hole), the critical radius is the **event horizon**:

```equation
equationrs = 2GM / c²
```

Where:

- `G` is the gravitational constant
- `M` is the black hole's mass
- `c` is the speed of light

We use geometric units where `G = c = 1`, so `rs = 2M`. With a mass of 1.0, our event horizon sits at radius 2.0.

### 1.2 How Light Bends

In general relativity, massive objects curve spacetime, and light follows these curves, called **geodesics**. Near a black hole, the bending is extreme. Light passing close to the event horizon can loop around multiple times before escaping.

This creates three key visual effects:

1. **Einstein rings** - Background stars appear as rings around the black hole
2. **Multiple images** - The same object visible from different angles
3. **The shadow** - A dark region where light has fallen in

### 1.3 The Accretion Disk

![black hole accretion disk](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767760863231.png)Black hole accretion disk visualization — NASA (https://svs.gsfc.nasa.gov/13326/) Matter falling into a black hole doesn't drop straight in. Conservation of angular momentum forces it into a flat, rotating disk called an **accretion disk**.

**The Inner Edge**

For a Schwarzschild black hole, the innermost stable circular orbit (ISCO) is at `r = 3 × rs`. Anything inside this spirals in. With `rs = 2`, that gives us `ISCO = 6.0`, but we often set the inner edge at `r = 3.0` for more dramatic visuals.

**Temperature Profile**

The inner disk runs hotter because gravitational potential energy converts to heat as matter falls inward.

The thin disk model predicts:

```equation
equationT ∝ r^(-3/4)
```

The inner edge glows white-hot while the outer edge a cooler reddish-orange. We'll use blackbody radiation to visualize this gradient.

**Keplerian Rotation:**

Disk material follows Kepler's laws - inner regions orbit faster than outer regions:

```equation
equationv ∝ r^(-1/2)
```

This differential rotation stretches any structures into arcs. It also creates a brightness asymmetry through Doppler beaming - the side moving toward us appears brighter.

---

## Part 2: Project Setup

### 2.1 Three.js with WebGPU and TSL

Three.js recently introduced **Three Shading Language (TSL)**, a JavaScript-based shader language that compiles to WGSL (WebGPU) or GLSL (WebGL). We write shaders in JavaScript syntax, which is convenient.

Here's the basic structure:

```javascript
javascriptimport * as THREE from "three/webgpu";
import { uniform } from "three/tsl";

// Initialize WebGPU renderer
const renderer = new THREE.WebGPURenderer({ antialias: true });
renderer.setSize(window.innerWidth, window.innerHeight);
document.body.appendChild(renderer.domElement);

// Create scene and camera
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(
  75,
  window.innerWidth / window.innerHeight,
  0.1,
  1000
);
camera.position.set(0, 5, 20);

// Create a large inverted sphere as our render surface
// By inverting it, we render from inside - perfect for a skybox-style shader
const geometry = new THREE.SphereGeometry(100, 32, 32);
geometry.scale(-1, 1, 1); // Invert the sphere

const material = new THREE.MeshBasicNodeMaterial();
// material.colorNode will hold our shader
```

### 2.2 Shader Organization

We'll structure our shader into four sections:

1. **Utility functions** - Hash functions, noise, FBM
2. **Background** - Stars and nebula
3. **Accretion disk** - Color, Doppler beaming, opacity
4. **Main raymarcher** - The core loop tracing light through curved spacetime

## Part 3: Procedural Background

Before we add the black hole, we need something for it to distort. A black hole in empty space wouldn't be very interesting—you'd just see blackness. The magic of gravitational lensing only becomes visible when there's a background to bend.

We'll build a procedural star field and optional nebula clouds. Later, when we add the raymarching loop with gravitational bending, we'll see these stars warp and stretch around the event horizon.

### 3.1 Utility Functions: Hash and Noise

Procedural generation relies on deterministic randomness. We use hash functions that produce consistent pseudo-random values for any input:

```javascript
javascript// 2D input -> 1D output hash
const hash21 = Fn(([p]) => {
  const n = sin(dot(p, vec2(127.1, 311.7))).mul(43758.5453);
  return fract(n);
});

// 2D input -> 2D output hash
const hash22 = Fn(([p]) => {
  const px = fract(sin(dot(p, vec2(127.1, 311.7))).mul(43758.5453));
  const py = fract(sin(dot(p, vec2(269.5, 183.3))).mul(43758.5453));
  return vec2(px, py);
});

// 3D input -> 1D output hash
const hash31 = Fn(([p]) => {
  const n = sin(dot(p, vec3(127.1, 311.7, 74.7))).mul(43758.5453);
  return fract(n);
});
```

For nebula clouds, we need smooth noise. Here's 3D value noise with trilinear interpolation:

```javascript
javascriptconst noise3D = Fn(([p]) => {
  const i = floor(p);
  const f = fract(p);
  // Smooth interpolation curve (equivalent to smoothstep)
  const u = f.mul(f).mul(float(3.0).sub(f.mul(2.0)));

  // Hash the 8 corners of the unit cube
  const a = hash31(i);
  const b = hash31(i.add(vec3(1, 0, 0)));
  const c = hash31(i.add(vec3(0, 1, 0)));
  const d = hash31(i.add(vec3(1, 1, 0)));
  const e = hash31(i.add(vec3(0, 0, 1)));
  const f2 = hash31(i.add(vec3(1, 0, 1)));
  const g = hash31(i.add(vec3(0, 1, 1)));
  const h = hash31(i.add(vec3(1, 1, 1)));

  // Trilinear interpolation
  return mix(
    mix(mix(a, b, u.x), mix(c, d, u.x), u.y),
    mix(mix(e, f2, u.x), mix(g, h, u.x), u.y),
    u.z
  );
});
```

**Fractal Brownian Motion (FBM)** layers multiple octaves of noise at different frequencies to create natural-looking patterns:

```javascript
javascriptconst fbm = Fn(([p, lacunarity, persistence]) => {
  const value = float(0.0).toVar();
  const amplitude = float(0.5).toVar();
  const pos = p.toVar();

  // 4 octaves of noise
  value.addAssign(noise3D(pos).mul(amplitude));
  pos.mulAssign(lacunarity);
  amplitude.mulAssign(persistence);

  value.addAssign(noise3D(pos).mul(amplitude));
  pos.mulAssign(lacunarity);
  amplitude.mulAssign(persistence);

  value.addAssign(noise3D(pos).mul(amplitude));
  pos.mulAssign(lacunarity);
  amplitude.mulAssign(persistence);

  value.addAssign(noise3D(pos).mul(amplitude));

  return value;
});
```

Where:

- `lacunarity` controls how quickly frequency increases (typically 2.0)
- `persistence` controls how quickly amplitude decreases (typically 0.5)

### 3.2 Star Field

We create stars using a grid-based approach. The idea: divide the sky into cells, and place one star (or none) in each cell based on a hash value.

```javascript
javascriptconst createStarField = (uniforms) =>
  Fn(([rayDir]) => {
    // Convert ray direction to spherical coordinates
    const theta = atan(rayDir.z, rayDir.x); // Azimuthal angle
    const phi = asin(clamp(rayDir.y, float(-1.0), float(1.0))); // Polar angle

    // Create grid cells across the sky
    const gridScale = float(60.0).div(uniforms.starSize);
    const scaledCoord = vec2(theta, phi).mul(gridScale);
    const cell = floor(scaledCoord);
    const cellUV = fract(scaledCoord); // Position within cell (0-1)

    // Decide if this cell has a star (based on density)
    const cellHash = hash21(cell);
    const starProb = step(float(1.0).sub(uniforms.starDensity), cellHash);

    // Random position within the cell (away from edges)
    const starPos = hash22(cell.add(42.0)).mul(0.8).add(0.1);
    const distToStar = length(cellUV.sub(starPos));

    // Star size varies per cell
    const baseSizeVar = hash21(cell.add(100.0)).mul(0.03).add(0.01);
    const finalStarSize = baseSizeVar.mul(uniforms.starSize);

    // Core + glow falloff
    const starCore = smoothstep(finalStarSize, float(0.0), distToStar);
    const starGlow = smoothstep(finalStarSize.mul(3.0), float(0.0), distToStar).mul(0.3);
    const starIntensity = starCore.add(starGlow).mul(starProb);

    // Slight color temperature variation
    const colorTemp = hash21(cell.add(200.0));
    const starColor = mix(vec3(0.8, 0.9, 1.0), vec3(1.0, 0.95, 0.8), colorTemp);

    return starColor.mul(starIntensity).mul(uniforms.starBrightness);
  });
```

To render the stars, we sample this function using the ray direction:

```javascript
javascriptconst starField = createStarField(uniforms);

// In the shader:
const bgColor = uniforms.starBackgroundColor.toVar("bgColor");
bgColor.addAssign(starField(rayDir));
return vec4(bgColor, 1.0);
```

At this point, we now have a simple star field on the screen. Next, let's create some subtle nebulae to give our black hole something to refract.

![star field](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767763212694.png) ### 3.3 Nebula Clouds

For additional atmosphere, we add procedural nebula using layered FBM. Two layers with different scales create depth:

```javascript
javascriptconst createNebulaField = (uniforms) =>
  Fn(([rayDir]) => {
    // Layer 1: Large-scale structure
    const noisePos1 = rayDir.mul(uniforms.nebula1Scale);
    const n1 = fbm(noisePos1, float(2.0), float(0.5)).mul(2.0).sub(1.0);
    const layer1 = clamp(n1.add(uniforms.nebula1Density), float(0.0), float(1.0));
    const color1 = uniforms.nebula1Color.mul(layer1).mul(uniforms.nebula1Brightness);

    // Layer 2: Finer detail at different scale
    const noisePos2 = rayDir.mul(uniforms.nebula2Scale);
    const n2 = fbm(noisePos2, float(2.0), float(0.5)).mul(2.0).sub(1.0);
    const layer2 = clamp(n2.add(uniforms.nebula2Density), float(0.0), float(1.0));
    const color2 = uniforms.nebula2Color.mul(layer2).mul(uniforms.nebula2Brightness);

    return color1.add(color2);
  });
```

Add the nebula to the background:

```javascript
javascriptconst nebulaField = createNebulaField(uniforms);

// In the shader:
If(uniforms.nebulaEnabled.greaterThan(0.5), () => {
  bgColor.addAssign(nebulaField(rayDir));
});
```

![nebula](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767763293184.png) Now we have a rich background. In the next section, we'll add the black hole and watch it bend this starfield into Einstein rings.

---

## Part 4: The Raymarching Core

### 4.1 Why Raymarching?

Standard rasterization renders objects by projecting triangles onto the screen. That doesn't work for gravitational lensing because we need to bend light in ways triangles can't represent. We need to trace each ray's path through curved spacetime.

**Raymarching** works by:

1. Creating a ray from the camera for each pixel
2. Stepping the ray forward in small increments
3. Bending the ray toward the black hole at each step
4. Checking if the ray hits the disk, falls into the hole, or escapes

### 4.2 Camera Setup

First, we generate rays for each pixel by building a camera coordinate system:

```javascript
javascript// Get UV coordinates centered at (0,0) ranging from -1 to 1
const uv = screenUV.sub(0.5).mul(2.0);
const aspect = uniforms.resolution.x.div(uniforms.resolution.y);
const screenPos = vec2(uv.x.mul(aspect), uv.y);

// Camera basis vectors
const camPos = uniforms.cameraPosition;
const camTarget = uniforms.cameraTarget;
const camForward = normalize(camTarget.sub(camPos));
const worldUp = vec3(0.0, 1.0, 0.0);
const camRight = normalize(cross(worldUp, camForward));
const camUp = cross(camForward, camRight);

// Generate ray direction through this pixel
const fov = float(1.0);
const rayDir = normalize(
  camForward.mul(fov).add(camRight.mul(screenPos.x)).add(camUp.mul(screenPos.y))
).toVar("rayDir");
```

### 4.3 The Basic Loop

The raymarching loop simulates a photon's journey through spacetime. Each iteration, we:

1. Check if the ray fell into the black hole (captured)
2. Check if the ray escaped to the background (escaped)
3. Bend the ray toward the black hole (gravity)
4. Step the ray forward

We track both the current and previous positions so we can detect when the ray crosses the accretion disk plane (Y = 0) and interpolate the exact crossing point.

```javascript
javascriptconst rayPos = camPos.toVar("rayPos");
const prevPos = camPos.toVar("prevPos");
const rs = uniforms.blackHoleMass.mul(2.0); // Event horizon radius

Loop(64, () => {
  const r = length(rayPos);

  // Captured by black hole?
  If(r.lessThan(rs.mul(1.01)), () => {
    captured.assign(1.0);
    Break();
  });

  // Escaped to infinity?
  If(r.greaterThan(100.0), () => {
    escaped.assign(1.0);
    Break();
  });

  // Gravitational bending (see next section)
  // ...

  // Save previous position and step forward
  prevPos.assign(rayPos);
  rayPos.addAssign(rayDir.mul(uniforms.stepSize));
});
```

Without gravitational bending, this traces straight lines. Let's add the physics.

### 4.4 Gravitational Light Bending

Here's where the black hole effect comes from. We bend rays toward the center using a simplified model based on the Schwarzschild metric. The acceleration of a photon toward the black hole follows:

```equation
equationa = -rs / r² × u
```

Where:

- `a` is the acceleration vector applied to the ray direction
- `rs` is the Schwarzschild radius (event horizon)
- `r` is the distance from the black hole center
- `u` is the unit vector pointing from the ray toward the black hole center

This is an inverse-square relationship—rays passing closer to the black hole bend more sharply. Here's the implementation:

```javascript
javascript// Direction from ray to black hole center (normalized)
const toCenter = rayPos.negate().div(r);

// Bend strength: stronger when closer (inverse square law)
const bendStrength = rs
  .div(r.mul(r))
  .mul(uniforms.stepSize)
  .mul(uniforms.gravitationalLensing);

// Apply bending to ray direction
rayDir.addAssign(toCenter.mul(bendStrength));
rayDir.assign(normalize(rayDir));

// Then step forward
prevPos.assign(rayPos);
rayPos.addAssign(rayDir.mul(uniforms.stepSize));
```

The `gravitationalLensing` uniform (default 1.5) lets us tune the bend strength. A value of 1.5 produces results that match the expected physics reasonably well.

### 4.5 Rendering the Background

Now comes the key insight: for escaped rays, we sample the star field using the **bent** ray direction, not the original direction. This is what creates the gravitational lensing effect:

```javascript
javascript// After the raymarching loop
If(escaped.greaterThan(0.5), () => {
  // rayDir has been bent by gravity - stars appear distorted!
  const bgColor = uniforms.starBackgroundColor.toVar("bgColor");
  bgColor.addAssign(starField(rayDir));

  If(uniforms.nebulaEnabled.greaterThan(0.5), () => {
    bgColor.addAssign(nebulaField(rayDir));
  });

  color.assign(bgColor);
});
```

Combining all of this, we end up with an ominous, pitch black disk on the screen (which is actually a sphere). Immediately outside of the event horizon of the black hole, we can see how the light from the starfield and nebula are warped due to the intense gravity.

![black hole with stars and nebula](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767764290765.png)
---

## Part 5: The Accretion Disk

### 5.1 Thin Disk Intersection

We model the accretion disk as an infinitely thin plane at Y = 0. Instead of volumetric sampling (which is also much more computationally intensive), we use **analytic plane intersection** - detecting when the ray crosses the disk plane:

```javascript
javascript// Did we cross the Y = 0 plane?
const crossedPlane = prevPos.y.mul(rayPos.y).lessThan(0.0);

If(crossedPlane, () => {
  // Linear interpolation to find exact crossing point
  const t = prevPos.y.negate().div(rayPos.y.sub(prevPos.y));
  const hitPos = mix(prevPos, rayPos, t);

  // Radial distance from center
  const hitR = sqrt(hitPos.x.mul(hitPos.x).add(hitPos.z.mul(hitPos.z)));

  // Is this within the disk bounds?
  const inDisk = hitR.greaterThan(innerR).and(hitR.lessThan(outerR));

  If(inDisk, () => {
    const hitAngle = atan(hitPos.z, hitPos.x);
    color.assign(diskColor);
  });
});
```

This approach is efficient - we only compute disk color when we hit it, and we get exact intersection points regardless of step size. This also preserves all of the sharp details of the accretion disk.

The accretion disk is now visible, albeit only as a solid color. However, we can already see a lot of interesting details:

![black hole with solid accretion disk](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767765452420.png) - The gravitational lensing effect causes photons from the back side of the accretion disk (behind the black hole) to warp around both the top AND bottom of the black hole. Trippy!
- This image also shows a good example of a partial Einstein ring (white arc above and below the black hole). The light from a star is warped uniformly around each side of the black hole, resulting in a concentric ring of light.

### 5.2 Blackbody Temperature Coloring

Real accretion disks glow based on their temperature. The color of a hot object follows **Planck's law** of blackbody radiation - at lower temperatures objects glow red, progressing through orange, yellow, white, and finally blue-white at extreme temperatures.

Rather than approximating this relationship with ad-hoc formulas, we use a scientifically accurate lookup table from Mitchell Charity's blackbody color data. This dataset was computed using the CIE 1931 2-degree color matching functions and properly converted to sRGB color space.

```javascript
javascript// Mitchell Charity Blackbody Colors (CIE 1931 2-deg, sRGB)
// Source: http://www.vendian.org/mncharity/dir3/blackbody/
const BLACKBODY_COLORS = {
  1000: [1, 0.0337, 0],      // Deep red
  2000: [1, 0.2647, 0.0033], // Orange-red
  3000: [1, 0.487, 0.1411],  // Orange
  4000: [1, 0.6636, 0.3583], // Yellow-orange
  5000: [1, 0.7992, 0.6045], // Yellow-white
  6000: [1, 0.9019, 0.8473], // Warm white
  6500: [1, 0.9436, 0.9621], // Near white (D65 illuminant)
  7000: [0.9337, 0.915, 1],  // Cool white
  8000: [0.7874, 0.8187, 1], // Blue-white
  10000: [0.6268, 0.7039, 1], // Light blue
  // ... continues to 40000K
};
```

The shader uses linear interpolation between table entries for smooth color transitions:

```javascript
javascriptconst blackbodyColor = Fn(([tempK]) => {
  const temp = clamp(tempK, float(1000.0), float(40000.0));
  // Linear interpolation between lookup table entries
  // ... (searches for bracketing temperatures and interpolates RGB)
  return vec3(r, g, b);
});
```

**Temperature Profile:**

The thin disk model predicts temperature falls off with radius following a power law:

```equation
equationT(r) = T_peak × (r_inner / r)^α
```

Where:

- `T(r)` is the temperature at radius `r`
- `T_peak` is the peak temperature at the inner edge (e.g., 10,000K)
- `r_inner` is the inner disk radius
- `α` is the temperature falloff exponent (typically 0.75 for thin disks, but you can increase this to get a more dramatic effect)

```javascript
javascriptconst peakTempK = uniforms.diskTemperature.mul(1000.0); // e.g., 10 -> 10,000K

// Power law falloff: T = T_peak × (r_inner / r)^α
const tempK = peakTempK.mul(pow(innerR.div(hitR), uniforms.temperatureFalloff));

const diskColor = blackbodyColor(tempK);
```

This ensures the inner edge is always hottest, with temperature decreasing monotonically outward - matching the physics of gravitational energy release in accretion disks.

![accretion disk with blackbody temperature](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772062449.png) ### 5.3 Edge Softening

Sharp disk boundaries look artificial. We add smooth falloff at both edges using a normalized radius and smoothstep blending:

```equation
equationr_norm = (r - r_inner) / (r_outer - r_inner)
falloff = smoothstep(0, s_inner, r_norm) × smoothstep(1, 1 - s_outer, r_norm)
```

Where:

- `r_norm` is the normalized radius (0 at inner edge, 1 at outer edge)
- `s_inner` is the inner edge softness (fraction of disk width)
- `s_outer` is the outer edge softness (fraction of disk width)
- `smoothstep` provides smooth Hermite interpolation between 0 and 1

```javascript
javascriptconst normR = clamp(
  hitR.sub(innerR).div(outerR.sub(innerR)),
  float(0.0),
  float(1.0)
);

const edgeFalloff = smoothstep(
  float(0.0),
  uniforms.diskEdgeSoftnessInner,
  normR
).mul(
  smoothstep(float(1.0), float(1.0).sub(uniforms.diskEdgeSoftnessOuter), normR)
);
```

### 5.4 Doppler Beaming

One of the most striking features of a real accretion disk is the brightness asymmetry from **Doppler beaming**. The side rotating toward us appears significantly brighter.

This happens because of relativistic effects:

- Light from approaching material is blueshifted and intensified
- Light from receding material is redshifted and dimmed
- The effect scales as D³ where D is the Doppler factor

The relativistic Doppler factor and beaming equations are:

```equation
equationD = 1 / (1 - β × cos(θ))
I_observed = I_emitted × D³
```

Where:

- `D` is the Doppler factor
- `β` is the velocity as a fraction of the speed of light (v/c)
- `θ` is the angle between the velocity vector and the ray direction
- `I_observed` is the observed intensity
- `I_emitted` is the emitted intensity
- The D³ factor comes from relativistic beaming (intensity transforms as frequency cubed)

For Keplerian orbits, the orbital velocity decreases with radius:

```equation
equationv ∝ 1 / √r
```

Here's the implementation:

```javascript
javascript// Material in Keplerian orbit moves tangentially
// Velocity direction: perpendicular to radial, in the rotation plane
const rotationSign = sign(uniforms.diskRotationSpeed);
const velocityDir = vec3(
  sin(hitAngle).negate().mul(rotationSign),
  float(0.0),
  cos(hitAngle).mul(rotationSign)
);

// Keplerian velocity: v ∝ 1/√r (inner disk moves faster)
const velocityMagnitude = float(1.0).div(sqrt(hitR.div(innerR)));

// Doppler factor: D = 1 / (1 - β·cos(θ))
// β is velocity as fraction of c, θ is angle between velocity and ray
const beta = velocityMagnitude.mul(0.3); // Scale for visual effect
const cosTheta = dot(velocityDir, rayDir);
const dopplerFactor = float(1.0).div(float(1.0).sub(beta.mul(cosTheta)));

// Apply D³ beaming (clamped to prevent extreme values)
const dopplerBoost = pow(
  dopplerFactor,
  float(3.0).mul(uniforms.dopplerStrength)
);
diskColor.mulAssign(clamp(dopplerBoost, float(0.1), float(5.0)));
```

When disk material moves toward the observer, `cosTheta < 0`, making `dopplerFactor > 1` and the disk brighter. When moving away, the disk dims.

![doppler beaming](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772927730.png) ### 5.5 Turbulent Ring Patterns

Real accretion disks have turbulent structure from magnetohydrodynamic instabilities. We create this using **3D Fractal Brownian Motion (FBM)**, which serves as our base noise function.

![fractal brownian motion](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772590634.png)2D noise generated via FBM First, the noise functions:

```javascript
javascript// 3D Value noise
const noise3D = Fn(([p]) => {
  const i = floor(p);
  const f = fract(p);
  const u = f.mul(f).mul(float(3.0).sub(f.mul(2.0))); // Smoothstep

  // Hash the 8 corners of the cube and interpolate
  // ... (trilinear interpolation of hashed values)
});

// Fractal Brownian Motion - layered noise
const fbm = Fn(([p, lacunarity, persistence]) => {
  const value = float(0.0).toVar();
  const amplitude = float(0.5).toVar();
  const pos = p.toVar();

  // 4 octaves
  for (let i = 0; i < 4; i++) {
    value.addAssign(noise3D(pos).mul(amplitude));
    pos.mulAssign(lacunarity);
    amplitude.mulAssign(persistence);
  }

  return value;
});
```

The key is using **anisotropic coordinates** - stretching the noise differently in radial vs. azimuthal directions to create arcs instead of random blobs. For Keplerian rotation, the angular velocity follows Kepler's third law:

```equation
equationω(r) = ω₀ / r^(3/2)
φ(t) = θ + ω(r) × t
```

Where:

- `ω(r)` is the angular velocity at radius `r`
- `ω₀` is a base rotation speed constant
- `φ(t)` is the rotated angle at time `t`
- `θ` is the initial azimuthal angle
- The `r^(3/2)` exponent comes from Kepler's third law (T² ∝ r³)

```javascript
javascriptconst hitAngle = atan(hitPos.z, hitPos.x);

// Keplerian rotation: inner regions rotate faster
const keplerianPhase = time
  .mul(uniforms.diskRotationSpeed)
  .div(pow(hitR, float(1.5)));
const rotatedAngle = hitAngle.add(keplerianPhase);

// Anisotropic sampling: radial creates rings, azimuthal creates arcs
const noiseCoord = vec3(
  hitR.mul(uniforms.turbulenceScale), // Radial component
  cos(rotatedAngle).div(uniforms.turbulenceStretch.max(0.1)), // Stretched azimuthally
  sin(rotatedAngle).div(uniforms.turbulenceStretch.max(0.1))
);

const turbulence = fbm(
  noiseCoord,
  uniforms.turbulenceLacunarity,
  uniforms.turbulencePersistence
);
const ringOpacity = pow(
  clamp(turbulence, float(0.0), float(1.0)),
  uniforms.turbulenceSharpness
);
```

The `turbulenceStretch` parameter controls how elongated the structures are. Higher values create longer arcs.

The result is an accretion disk that looks much more realistic with interesting details at multiple scales. This gives the impression of irregular clumps of stellar material and dust.

![turbulence](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767773797967.png) ### 5.6 The Cyclic Time Problem

There's a subtle issue with Keplerian rotation. Recall the differential rotation equation:

```equation
equationω(r) = ω₀ / r^(3/2)
```

The inner disk rotates faster than the outer disk. This means any structure in the disk gets progressively wound up—stretched into tighter and tighter spirals as inner material laps outer material.

At startup, the turbulence shows natural-looking arc structures.

![initial noise pattern](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775234303.png) After ~30 seconds, differential rotation has wound the patterns into unrealistically tight spirals.

![noise pattern after 30 seconds](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775168736.png) **Why this isn't realistic:** In a real accretion disk, turbulence doesn't just wind up forever. Magnetohydrodynamic instabilities (MRI) constantly generate new turbulent structures that break apart and reform. The disk maintains a characteristic turbulence scale rather than winding into infinitely thin filaments.

The fix is **cyclic time with crossfading**. We reset the time every N seconds—but a sudden reset would cause a visible pop. We hide this by running two noise samples: one at `t` and another at `t + N`. We slowly crossfade from sample 1 to sample 2 over N seconds. The transition is slow enough to not be noticeable and also creates some time-varying changes in the turbulence which adds to the effect.

```javascript
javascriptconst cycleLength = uniforms.turbulenceCycleTime; // e.g., 10 seconds
const cyclicTime = time.mod(cycleLength);
const blendFactor = cyclicTime.div(cycleLength);

// Sample two phases offset by one cycle
const phase1 = cyclicTime.mul(rotationSpeed).div(pow(hitR, float(1.5)));
const phase2 = cyclicTime
  .add(cycleLength)
  .mul(rotationSpeed)
  .div(pow(hitR, float(1.5)));

const turbulence1 = fbm(/* coords with phase1 */);
const turbulence2 = fbm(/* coords with phase2 */);

// Crossfade to hide the cycle reset
const turbulence = mix(turbulence2, turbulence1, blendFactor);
```

### 5.7 Alpha Compositing

The ray can cross the disk plane multiple times due to lensing, so we use front-to-back alpha compositing:

```javascript
javascriptconst color = vec3(0.0, 0.0, 0.0).toVar("color");
const alpha = float(0.0).toVar("alpha");

// When we hit the disk:
const diskResult = accretionDiskColor(hitR, hitAngle, time, rayDir);
const remainingAlpha = float(1.0).sub(alpha);
color.addAssign(diskResult.xyz.mul(diskResult.w).mul(remainingAlpha));
alpha.addAssign(remainingAlpha.mul(diskResult.w));

// Early termination when fully opaque
If(alpha.greaterThan(0.99), () => {
  Break();
});
```

## The Final Product

![final black hole shader](https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775626979.png) And with that, our shader is complete! We've built a real-time black hole visualization from scratch.

The key takeaways:

1.
**Raymarching handles curved spacetime well** - we bend rays at each step according to gravitational physics.

2.
**Analytic disk intersection is efficient** - for a thin disk, detecting zero-crossings gives exact hit points

3.
**Doppler beaming creates realistic asymmetry** - the approaching side glows brighter due to relativistic effects, matching real observations.

4.
**Anisotropic noise creates believable structure** - stretching noise differently in radial vs. azimuthal directions produces arc-like patterns.

5.
**Blackbody radiation gives physically-motivated color** - the temperature profile naturally produces the white-hot inner edge fading to red-orange outer regions.

The effect is most dramatic when you orbit the camera around the black hole. Watch how the disk appears to bend above and below, with the far side visible through gravitational lensing. Background stars create Einstein rings as they pass behind. The Doppler beaming makes one side dramatically brighter, just like in real astronomical observations.

If you enjoyed this tutorial, be sure to check out my [galaxy simulation](https://threejsroadmap.com/blog/galaxy-simulation-webgpu-compute-shaders) tutorial and my [realistic water shader course](https://threejsroadmap.com/courses/realistic-water).

Thanks for reading!

—Dan

---

## References

1.
James, O., von Tunzelmann, E., Franklin, P., & Thorne, K. S. (2015). *Gravitational lensing by spinning black holes in astrophysics, and in the movie Interstellar*. Classical and Quantum Gravity.

2.
Schwarzschild, K. (1916). *On the gravitational field of a mass point according to Einstein's theory*.

3.
Shakura, N. I., & Sunyaev, R. A. (1973). *Black holes in binary systems. Observational appearance*.

4.
Three.js TSL Documentation: [https://github.com/mrdoob/three.js/wiki/Three.js-Shading-Language](https://github.com/mrdoob/three.js/wiki/Three.js-Shading-Language)

---

[Source Code](https://github.com/dgreenheck/webgpu-black-hole) • [Live Demo](https://dgreenheck.github.io/webgpu-black-hole/)

Find any mistakes or bugs in this article? Please let me know by sending an email to [support@threejsroadmap.com](https://threejsroadmap.com/blog/mailto:support@threejsroadmap.com).

### Enjoying the blog?

Subscribe to the Three.js Roadmap newsletter for new posts, course releases, and exclusive discounts.

SubscribeUnsubscribe anytime. No spam.

## Links

- [Home](https://threejsroadmap.com/)
- [Assets](https://threejsroadmap.com/assets)
- [Courses](https://threejsroadmap.com/courses)
- [FAQ](https://threejsroadmap.com/#faq)

## Legal

- [Privacy Policy](https://threejsroadmap.com/privacy-policy)
- [Cookie Policy](https://threejsroadmap.com/cookie-policy)
- [Terms of Service](https://threejsroadmap.com/terms-of-service)

## Contact

[support@threejsroadmap.com](https://threejsroadmap.com/blog/mailto:support@threejsroadmap.com)[X](https://x.com/dangreenheck)[LinkedIn](https://linkedin.com/in/danielgreenheck/)[Instagram](https://www.instagram.com/dan.greenheck/)[YouTube](https://www.youtube.com/@dangreenheck)[GitHub](https://github.com/dgreenheck)© 2026 Three.js Roadmap. All rights reserved.

[Privacy Settings](https://threejsroadmap.com/blog/javascript:throw%20new%20Error%28'React%20has%20blocked%20a%20javascript:%20URL%20as%20a%20security%20precaution.'%29)

## Media links

- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767759963539.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767760863231.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767763212694.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767763293184.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767764290765.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767765452420.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772062449.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772927730.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767772590634.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767773797967.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775234303.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775168736.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767775626979.png>
- <https://jdhzrefjyfmtivbsmtnw.supabase.co/storage/v1/object/public/blog_images/blog/1767759011801.png>
- <https://threejsroadmap.com/favicon-32x32.png>
- <https://threejsroadmap.com/favicon-16x16.png>
- <https://threejsroadmap.com/apple-touch-icon.png>
- <https://threejsroadmap.com/images/threejs-roadmap-logo_512.png>
