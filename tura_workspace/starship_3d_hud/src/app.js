import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { EffectComposer } from "three/addons/postprocessing/EffectComposer.js";
import { RenderPass } from "three/addons/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/addons/postprocessing/UnrealBloomPass.js";
import { ShaderPass } from "three/addons/postprocessing/ShaderPass.js";
import { FXAAShader } from "three/addons/shaders/FXAAShader.js";

const canvas = document.querySelector("#stage");
const scene = new THREE.Scene();
scene.fog = new THREE.FogExp2(0x020713, 0.026);

const camera = new THREE.PerspectiveCamera(48, window.innerWidth / window.innerHeight, 0.1, 900);
camera.position.set(0, 2.2, 14.2);

const renderer = new THREE.WebGLRenderer({
  canvas,
  antialias: true,
  alpha: false,
  powerPreference: "high-performance",
});
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.75));
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.14;

const composer = new EffectComposer(renderer);
composer.addPass(new RenderPass(scene, camera));

const bloom = new UnrealBloomPass(
  new THREE.Vector2(window.innerWidth, window.innerHeight),
  0.54,
  0.46,
  0.24,
);
composer.addPass(bloom);

const fxaa = new ShaderPass(FXAAShader);
composer.addPass(fxaa);

const loader = new GLTFLoader();
const textureLoader = new THREE.TextureLoader();
const clock = new THREE.Clock();
const pointer = new THREE.Vector2();
const targetPointer = new THREE.Vector2();
const groups = {
  fleet: new THREE.Group(),
  effects: new THREE.Group(),
  orbital: new THREE.Group(),
};

scene.add(groups.fleet, groups.effects, groups.orbital);

const ui = {
  vector: document.querySelector("#vector-readout"),
  thrust: document.querySelector("#thrust-readout"),
  shield: document.querySelector("#shield-readout"),
  range: document.querySelector("#range-readout"),
  solution: document.querySelector("#solution-readout"),
  clock: document.querySelector("#pulse-clock"),
  target: document.querySelector("#target-label"),
};

if (window.lucide) {
  window.lucide.createIcons();
}

document.querySelectorAll(".mode").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".mode").forEach((item) => item.classList.remove("active"));
    button.classList.add("active");
    setMode(button.dataset.mode);
  });
});

function setMode(mode) {
  if (mode === "cloak") {
    ui.target.textContent = "LOW SIGNATURE";
    bloom.strength = 0.52;
    renderer.toneMappingExposure = 0.96;
  } else if (mode === "boost") {
    ui.target.textContent = "DRIVE SURGE";
    bloom.strength = 0.76;
    renderer.toneMappingExposure = 1.22;
  } else {
    ui.target.textContent = "HOSTILE CARRIER";
    bloom.strength = 0.54;
    renderer.toneMappingExposure = 1.14;
  }
}

window.addEventListener("pointermove", (event) => {
  targetPointer.x = (event.clientX / window.innerWidth - 0.5) * 2;
  targetPointer.y = (event.clientY / window.innerHeight - 0.5) * 2;
});

window.addEventListener("resize", resize);

setupLighting();
setupStarfield();
setupParticleField();
setupEnergyEffects();
await loadFleet();
animate();

function setupLighting() {
  const key = new THREE.DirectionalLight(0x9bd8ff, 3.2);
  key.position.set(-4, 8, 8);
  scene.add(key);

  const rim = new THREE.DirectionalLight(0xff6648, 2.4);
  rim.position.set(5, -2, -7);
  scene.add(rim);

  const fill = new THREE.HemisphereLight(0x1e65ff, 0x07020a, 1.25);
  scene.add(fill);

  const beacon = new THREE.PointLight(0xff624c, 12, 28);
  beacon.position.set(0, 0.7, -5);
  groups.effects.add(beacon);
}

function setupStarfield() {
  textureLoader.load("./assets/starfield.jpg", (texture) => {
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.wrapS = THREE.RepeatWrapping;
    texture.repeat.x = -1;

    const sky = new THREE.Mesh(
      new THREE.SphereGeometry(360, 48, 24),
      new THREE.MeshBasicMaterial({
        map: texture,
        side: THREE.BackSide,
        color: new THREE.Color(0x9fb3ff),
      }),
    );
    sky.rotation.y = Math.PI * 0.72;
    sky.rotation.z = -0.12;
    scene.add(sky);
  });
}

function setupParticleField() {
  const count = 1400;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  const color = new THREE.Color();

  for (let i = 0; i < count; i += 1) {
    const spread = 130;
    positions[i * 3] = (Math.random() - 0.5) * spread;
    positions[i * 3 + 1] = (Math.random() - 0.5) * 52;
    positions[i * 3 + 2] = -Math.random() * 180 + 25;

    color.setHSL(0.56 + Math.random() * 0.08, 0.85, 0.62 + Math.random() * 0.22);
    colors[i * 3] = color.r;
    colors[i * 3 + 1] = color.g;
    colors[i * 3 + 2] = color.b;
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));

  const material = new THREE.PointsMaterial({
    size: 0.075,
    vertexColors: true,
    transparent: true,
    opacity: 0.78,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });

  const stars = new THREE.Points(geometry, material);
  stars.name = "parallax-star-particles";
  groups.effects.add(stars);
}

function setupEnergyEffects() {
  for (let i = 0; i < 5; i += 1) {
    const beam = createBeam(8 + i * 0.8, i % 2 === 0 ? 0x7fe3ff : 0xff624c);
    beam.position.set((i - 2) * 1.1, -0.36 - Math.random() * 0.34, -2.5 - i * 1.6);
    beam.rotation.z = (i - 2) * 0.035;
    groups.effects.add(beam);
  }

  const shield = new THREE.Mesh(
    new THREE.TorusGeometry(4.2, 0.012, 6, 180),
    new THREE.MeshBasicMaterial({
      color: 0x7fe3ff,
      transparent: true,
      opacity: 0.18,
      blending: THREE.AdditiveBlending,
    }),
  );
  shield.name = "target-lock-ring";
  shield.rotation.x = Math.PI / 2;
  shield.position.set(0, 0.1, -0.8);
  groups.effects.add(shield);
}

function createBeam(length, color) {
  const geometry = new THREE.CylinderGeometry(0.018, 0.12, length, 12, 1, true);
  geometry.rotateX(Math.PI / 2);
  geometry.translate(0, 0, -length / 2);
  return new THREE.Mesh(
    geometry,
    new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.38,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    }),
  );
}

async function loadFleet() {
  const [flagship, wingman, hostile, planet, base] = await Promise.all([
    loadModel("./assets/wingman.glb"),
    loadModel("./assets/starship.glb"),
    loadModel("./assets/enemy_large.glb"),
    loadModel("./assets/planet.glb"),
    loadModel("./assets/base_large.glb"),
  ]);

  normalizeModel(flagship, 6.4);
  flagship.position.set(0, -0.4, 0.3);
  flagship.rotation.set(-0.14, Math.PI * 0.96, -0.05);
  remapMaterials(flagship, {
    metal: 0x8fa1b8,
    accent: 0x315fd5,
    emissive: 0x48c7ff,
  });
  groups.fleet.add(flagship);

  const wingLeft = wingman.clone(true);
  normalizeModel(wingLeft, 1.9);
  wingLeft.position.set(-5.5, -0.8, -6);
  wingLeft.rotation.set(-0.06, Math.PI * 0.96, 0.12);
  remapMaterials(wingLeft, { metal: 0x8ea7c8, accent: 0x7fe3ff, emissive: 0x7fe3ff });

  const wingRight = wingman.clone(true);
  normalizeModel(wingRight, 1.55);
  wingRight.position.set(5.2, -1.05, -7.2);
  wingRight.rotation.set(-0.03, Math.PI * 1.04, -0.15);
  remapMaterials(wingRight, { metal: 0xa0aac4, accent: 0xf6b75d, emissive: 0xf6b75d });

  groups.fleet.add(wingLeft, wingRight);

  normalizeModel(hostile, 2.4);
  hostile.position.set(8, 1.6, -18);
  hostile.rotation.set(0.1, Math.PI * 1.2, -0.24);
  remapMaterials(hostile, { metal: 0x583848, accent: 0xff624c, emissive: 0xff624c });
  groups.orbital.add(hostile);

  normalizeModel(planet, 16);
  planet.position.set(-18, -6.5, -42);
  planet.rotation.set(0.2, -0.6, 0.15);
  remapMaterials(planet, { metal: 0x365c8e, accent: 0x7fe3ff, emissive: 0x12345f });
  groups.orbital.add(planet);

  normalizeModel(base, 4.4);
  base.position.set(-9.5, -4.4, -18);
  base.rotation.set(-0.1, 0.64, 0.12);
  remapMaterials(base, { metal: 0x8795a8, accent: 0x3c74ff, emissive: 0xff624c });
  groups.orbital.add(base);

  addEngineGlow(flagship, new THREE.Vector3(-0.1, -0.28, 1.55), 0.42);
  addEngineGlow(wingLeft, new THREE.Vector3(0, 0, 0.9), 0.34);
  addEngineGlow(wingRight, new THREE.Vector3(0, 0, 0.9), 0.3);
}

function loadModel(url) {
  return new Promise((resolve, reject) => {
    loader.load(url, (gltf) => resolve(gltf.scene), undefined, reject);
  });
}

function normalizeModel(model, targetSize) {
  const box = new THREE.Box3().setFromObject(model);
  const size = new THREE.Vector3();
  const center = new THREE.Vector3();
  box.getSize(size);
  box.getCenter(center);

  const max = Math.max(size.x, size.y, size.z) || 1;
  model.scale.multiplyScalar(targetSize / max);
  model.position.sub(center.multiplyScalar(targetSize / max));
}

function remapMaterials(model, palette) {
  let index = 0;
  model.traverse((child) => {
    if (!child.isMesh) return;

    child.castShadow = true;
    child.receiveShadow = true;

    const hueColor = index % 4 === 0 ? palette.accent : palette.metal;
    child.material = new THREE.MeshStandardMaterial({
      color: index % 7 === 0 ? palette.accent : palette.metal,
      roughness: 0.42,
      metalness: 0.72,
      emissive: index % 11 === 0 ? palette.emissive : 0x000000,
      emissiveIntensity: index % 11 === 0 ? 0.12 : 0,
    });
    index += 1;
  });
}

function addEngineGlow(parent, position, scale) {
  const core = new THREE.Mesh(
    new THREE.SphereGeometry(scale, 24, 12),
    new THREE.MeshBasicMaterial({
      color: 0x7fe3ff,
      transparent: true,
      opacity: 0.5,
      blending: THREE.AdditiveBlending,
    }),
  );
  core.position.copy(position);
  parent.add(core);

  const trail = createBeam(5.2 * scale, 0x3c74ff);
  trail.position.copy(position).add(new THREE.Vector3(0, 0, 0.55 * scale));
  trail.scale.setScalar(scale);
  parent.add(trail);
}

function animate() {
  const elapsed = clock.getElapsedTime();
  pointer.lerp(targetPointer, 0.045);

  groups.fleet.rotation.y = pointer.x * 0.085 + Math.sin(elapsed * 0.35) * 0.018;
  groups.fleet.rotation.x = -pointer.y * 0.04 + Math.sin(elapsed * 0.42) * 0.012;
  groups.fleet.position.y = Math.sin(elapsed * 0.8) * 0.08;

  groups.orbital.rotation.y = elapsed * 0.018;
  groups.effects.children.forEach((child, index) => {
    if (child.name === "parallax-star-particles") {
      child.position.z = (elapsed * 4) % 16;
      child.rotation.y = pointer.x * 0.04;
      return;
    }
    if (child.name === "target-lock-ring") {
      child.rotation.z = elapsed * 0.9;
      child.scale.setScalar(1 + Math.sin(elapsed * 2.2) * 0.035);
      return;
    }
    child.material?.opacity && (child.material.opacity = 0.18 + Math.sin(elapsed * 4 + index) * 0.055);
  });

  camera.position.x = pointer.x * 0.92;
  camera.position.y = 2.2 - pointer.y * 0.34;
  camera.lookAt(0, 0.08, -1.3);

  updateHud(elapsed);
  composer.render();
  requestAnimationFrame(animate);
}

function updateHud(elapsed) {
  const thrust = 84 + Math.sin(elapsed * 2.1) * 5;
  const shield = 91 + Math.sin(elapsed * 1.3 + 1.7) * 3;
  const solution = 66 + Math.sin(elapsed * 1.5) * 12;
  const range = 18.2 + Math.sin(elapsed * 0.7) * 0.7;
  const seconds = Math.floor(elapsed + 42);

  ui.vector.textContent = `${(34.8 + pointer.x * 4).toFixed(1)} / ${(-12.4 - pointer.y * 3).toFixed(1)}`;
  ui.thrust.textContent = `${Math.round(thrust)}%`;
  ui.shield.textContent = `${Math.round(shield)}%`;
  ui.solution.textContent = `${Math.round(solution)}%`;
  ui.range.textContent = `${range.toFixed(1)} km`;
  ui.clock.textContent = `T+ 00:${String(seconds % 60).padStart(2, "0")}`;
}

function resize() {
  const width = window.innerWidth;
  const height = window.innerHeight;

  camera.aspect = width / height;
  camera.updateProjectionMatrix();

  renderer.setSize(width, height);
  composer.setSize(width, height);
  fxaa.material.uniforms.resolution.value.x = 1 / (width * renderer.getPixelRatio());
  fxaa.material.uniforms.resolution.value.y = 1 / (height * renderer.getPixelRatio());
}

resize();
