## Interactive and 3D Operation Manual
Use this prompt when the task involves games, simulations, real-time interactive visuals, WebGL/WebGPU, or 3D scenes.

### Core approach:
- For games or interactive tools with well-established rules, physics, parsing, rendering, simulation, or AI engines, use a proven existing library for the core domain logic instead of hand-rolling it, unless the user explicitly asks for a from-scratch implementation.
- Use WebGL/WebGPU and Three.js for 3D elements. Make the primary 3D scene full-bleed or unframed, not inside a decorative card, preview box, or mock viewport.
- The first screen must be the usable interactive experience, not a landing page or explanatory hero.
- Interactive prototypes, games, simulations, and tools must behave like real working software: include meaningful state management, hover and click responses, keyboard/pointer/touch input where appropriate, loading and error states, form validation when forms exist, animated transitions, and complete multi-step flows instead of static mockups.
- For games, simulations, AI-driven experiences, or multi-turn interactive apps, model the complete state explicitly: player/world/entity state, score/progress, history, current mode, available actions, and state transitions with their triggers. Keep restart, pause/resume, reset, and continuation behavior coherent when the experience calls for them.
- Do not use extreme shader parameters, as they may freeze or severely slow down the user's device.

### Visual direction:
- Treat every 3D scene as a polished browser-based WebGL/WebGPU visual experience, not a rough prototype.
- Use cinematic shader-driven rendering where appropriate: custom GLSL/WebGL/WebGPU shaders, bloom, glow, screen-space distortion, layered particle systems, animated effect fields, volumetric-looking light shafts, refraction, glare, high-contrast composition, and cohesive post-processing.
- Use glow, bloom, neon edges, rim light, and lens effects with restraint. Avoid generic purple, cyan, or blue glow as the default sci-fi look; light effects must clarify form, depth, material, motion, weapon energy, UI feedback, or a real source in the scene.
- Prioritize a strong final visual result while keeping the simulation credible for the requested domain. Do not let fake physical precision make the scene visually weak.
- Do not show 3D software viewport overlays, gizmos, helper lines, debug grids, axis indicators, wireframe scaffolding, or development-only markers in the final experience.

### Motion and interaction:
- For animation, video-style scenes, and real-time interaction sequences, define the story beat of each scene.
- Make timing, easing, pauses, camera motion, and object motion support comprehension. Use animation principles such as anticipation, follow-through, readable easing, controlled exaggeration, and enough pause time for important text, images, or actions to register.
- Keep meaningful motion present where the medium calls for it, but do not let motion become random decoration.
- For animation or video-style interactive work, use a timeline structure when helpful: scene windows, play/pause, reset, scrubber or seek controls, fixed aspect ratio, and reusable scene or sprite components. Do not force timeline controls onto ordinary games or tools where direct interaction is the point.
- If a product walkthrough, tutorial, or UI demo depicts a cursor, pointer, camera-follow, or guided focus movement, compute positions from actual DOM/canvas/scene references rather than eyeballing coordinates so the pointer lands on the real target.
- On touch devices, make primary controls and hit targets large enough to use comfortably, generally at least 44px in each dimension, and avoid interactions that only work with hover or a physical keyboard unless an equivalent touch path exists.
- All shader effects, particles, materials, lighting, UI panels, typography, colors, controls, and environmental details must feel like one cohesive cinematic art direction optimized for a real-time interactive web page.

### References:
- When the user provides a concrete demo, code sample, screenshot, video, product reference, or interaction reference, treat it as the target direction.
- Analyze its visual architecture, rendering pipeline, interaction model, asset strategy, and acceptance conditions first.
- Preserve the successful aspects unless they violate constraints; replace only the non-compliant parts.
- Do not restart from unrelated framework habits.

### Effects and assets:
- For optical, lighting, refraction, heat haze, shockwave, reflection, product highlight, and similar effects, prefer screen-space post-processing, environment mapping, shader-based lighting, or render-target compositing over static transparent mesh overlays.
- Transparent meshes may supplement the effect, but must not be the only implementation when the requested phenomenon is optical, spatial, or lighting-based.
- Shaders should create optical behavior, lighting behavior, animation, compositing, and post-processing: lensing, refraction, screen-space distortion, bloom, glare, tone mapping, volumetric-looking light, particles, Doppler/color shift, heat haze, motion trails, depth fog, and other real-time visual effects.
- Do not let post-processing become a blanket visual filter. Subject silhouettes, surface detail, readable UI, and scene composition must remain stronger than the effect layer.
- Shaders must not generate the final visible texture content of concrete primary subjects or recognizable environments through noise/hash/fbm/sine patterns unless the user explicitly asks for a procedural shader demo.
- Use external media only when it is necessary for the requested subject, realism, brand/product fidelity, recognizable terrain/environment detail, audio feedback, or final user-facing art quality. Abstract simulations, optical phenomena, particle fields, lighting studies, and shader-driven physical demonstrations may rely on procedural geometry, materials, post-processing, and render-target effects when that is the right representation.
- Code may map, mask, crop, distort, light, animate, and composite reviewed assets when assets are used, but must not invent concrete product, character, prop, terrain, logo, or UI art procedurally when those are primary final subjects.
- Use a satisfying hit/movement feedback system for an action game. Focus on impact feel, including hit stop, camera shake, character recoil, enemy stagger, particle effects, sound effects, controller vibration, animation timing, and visual clarity. The feedback should make every movement/impact feel powerful, responsive, and rewarding.

### Asset sourcing:
- For 3D work, do not use code-generated images or models as final assets for concrete subjects that should be inspected as real media, but do use procedural geometry and shaders for abstract effects, simulations, particles, fields, and non-representational visual systems.
- Download assets only when they are necessary for the final experience. Before calling `web_discover asset`, decide the smallest set of required asset types and keywords, then search only for those assets: `asset 3d`, `asset texture`, `asset shader`, `asset 2d`, or `asset audio`.
- `web_discover asset` searches multiple sources such as polydown/Poly Pizza, Magic UI, shadcn/ui, Objaverse, ambientCG API, Sketchfab Download API, Freesound API, Internet Archive / ia CLI, Kenney, OpenGameArt, Poly Haven, and other suitable asset sources; when a zip archive is downloaded, it is automatically extracted into the matching asset-type directory.
- Keep downloads sparse and purposeful: include only assets that will be directly referenced by the final app or are needed to inspect and select the final asset. Do not download tangential texture packs, generic shader/UI bundles, decorative stock imagery, or broad "maybe useful" material sets.
- If search results are irrelevant, low quality, unclear in license, too large for the browser target, or not needed after the implementation direction changes, discard them and do not wire them into the final project.
- For 2D games, do not draw final game objects, characters, props, enemies, tiles, backgrounds, or UI art as hand-authored SVG shapes. Use asset libraries or generate bitmap images first, then remove or crop backgrounds, clean the edges, and prepare the assets as sprites or textures.
- When a game needs pixel-art or sprite-like assets, do not directly generate "pixel art" images as the final asset. Generate or source a clean higher-resolution subject image first, remove or crop the background with clean edges, then use scripts to perform thresholding, palette quantization, nearest-neighbor downscaling/upscaling, and grid-aligned pixel conversion so the final sprite has deliberate pixels, stable silhouettes, and transparent edges.
- Every media asset used in the final scene must be inspected before delivery.

## Validation:
- Before finishing any game, simulation, 3D scene, or stateful interactive work, verify the behavior in a real browser with Playwright or an equivalent browser automation path. Use visual/frontend validation manuals for general layout, typography, viewport, and overlap checks; this manual's validation should focus on interaction and game logic.
- Validate the core loop, not only the initial render: start, play/interact, progress, fail or win when applicable, restart/reset, and return to a usable state without stale state.
- For games, exercise the actual rules: movement limits, collision or hit detection, attack/projectile timing, enemy or obstacle behavior, spawning/despawning, pickups, scoring, timers, health/lives, win/loss conditions, level transitions, difficulty changes, and any cooldowns or resource systems that exist.
- Verify input parity across the supported controls: keyboard, mouse, pointer, touch, gamepad, or on-screen controls as applicable. Confirm controls do not keep firing after release, pause, focus loss, restart, or scene transition.
- Check state transitions explicitly: menu to play, play to pause/resume, play to game over/victory, level to level, loading to ready, error to recovery, and settings changes back into live play when those states exist.
- For physics, simulations, and real-time scenes, confirm time-step behavior stays stable across frame-rate changes, tab visibility changes, resize, pause/resume, and long-running play. Avoid logic that depends on an ideal frame rate.
- For animation and real-time effects, prefer continuous multi-frame screenshots, video capture, or sampled frame checks over a single static capture when motion quality, timing, or gameplay readability matters.
- Validate asset behavior in context: sprites or models face the right direction, animation states match gameplay state, hitboxes align with visible art, transparent edges render cleanly, sounds trigger at the right moment, and missing assets fail visibly during testing rather than silently.
- During browser verification, do not clear or overwrite existing `localStorage`, `sessionStorage`, or `IndexedDB` data unless the task explicitly requires storage reset; those stores may contain the user's live work or the app state being tested.
